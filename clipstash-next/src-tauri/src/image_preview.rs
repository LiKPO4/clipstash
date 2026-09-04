use crate::{app_data, image_thumbnails::validate_current_image_path};
use image::{metadata::Orientation, ImageDecoder, ImageReader};
use serde::Serialize;
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};
use tauri::Manager;

static UPLOAD: Mutex<Option<(String, PathBuf)>> = Mutex::new(None);
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const MAX_UPLOAD_BYTES: usize = 128 * 1024 * 1024;

#[derive(Serialize)]
pub struct PreviewSource {
    path: String,
    width: u32,
    height: u32,
    lease: Option<String>,
}

fn dimensions(path: &Path) -> Result<(u32, u32), String> {
    let decoder = ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| format!("读取预览图片尺寸失败：{e}"))?;
    oriented_dimensions(decoder)
}

fn oriented_dimensions(mut decoder: impl ImageDecoder) -> Result<(u32, u32), String> {
    let (width, height) = decoder.dimensions();
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    Ok(
        if matches!(
            orientation,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Rotate90FlipH
                | Orientation::Rotate270FlipH
        ) {
            (height, width)
        } else {
            (width, height)
        },
    )
}

fn grant_file_scope(
    already_allowed: bool,
    allow: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if already_allowed {
        return Ok(());
    }
    allow()
}

fn grant_asset_file_scope(scope: &tauri::scope::fs::Scope, path: &Path) -> Result<(), String> {
    grant_file_scope(scope.is_allowed(path), || {
        scope.allow_file(path).map_err(|e| e.to_string())
    })
}

#[tauri::command(async)]
pub async fn prepare_image_preview(
    app: tauri::AppHandle,
    filename: String,
    expected_path: String,
) -> Result<PreviewSource, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data_dir = app_data::ready_app_data_dir_path()?;
        let (_, path) = validate_current_image_path(&data_dir, &filename, &expected_path)?;
        let (width, height) = dimensions(&path)?;
        grant_asset_file_scope(&app.asset_protocol_scope(), &path)?;
        Ok(PreviewSource {
            path: path.to_string_lossy().into_owned(),
            width,
            height,
            lease: None,
        })
    })
    .await
    .map_err(|err| format!("图片预览任务意外中断：{err}"))?
}

fn save_upload(root: &Path, bytes: &[u8]) -> Result<PreviewSource, String> {
    if bytes.is_empty() || bytes.len() > MAX_UPLOAD_BYTES {
        return Err("临时预览图片为空或超过 128MiB".into());
    }
    let decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_decoder()
        .map_err(|e| format!("识别预览图片失败：{e}"))?;
    let (width, height) = oriented_dimensions(decoder)?;
    let mut current = UPLOAD.lock().map_err(|_| "预览临时文件锁已损坏")?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join("clipstash-preview-upload.bin");
    // Never follow a link at the app-owned temporary file name.
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err("预览临时文件路径不安全".into());
        }
    }
    fs::write(&path, bytes).map_err(|e| format!("暂存预览图片失败：{e}"))?;
    let lease = format!(
        "{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    *current = Some((lease.clone(), path.clone()));
    Ok(PreviewSource {
        path: path.to_string_lossy().into_owned(),
        width,
        height,
        lease: Some(lease),
    })
}

#[tauri::command]
pub async fn prepare_preview_upload(
    app: tauri::AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<PreviewSource, String> {
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) if bytes.len() <= MAX_UPLOAD_BYTES => bytes.clone(),
        _ => return Err("预览图片需要不超过 128MiB 的二进制数据".into()),
    };
    let root = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let source = save_upload(&root, &bytes)?;
        if let Err(err) =
            grant_asset_file_scope(&app.asset_protocol_scope(), Path::new(&source.path))
        {
            if let Some(lease) = &source.lease {
                let _ = release_preview_upload(lease.clone());
            }
            return Err(err);
        }
        Ok(source)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command(async)]
pub fn release_preview_upload(lease: String) -> Result<(), String> {
    let mut current = UPLOAD.lock().map_err(|_| "预览临时文件锁已损坏")?;
    if let Some((token, path)) = current.as_ref() {
        if token == &lease {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("清理预览临时文件失败：{e}")),
            }
            *current = None;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jpeg_preview_dimensions_follow_exif_rotation() {
        let mut encoded = Cursor::new(Vec::new());
        image::RgbImage::new(20, 40)
            .write_to(&mut encoded, image::ImageFormat::Jpeg)
            .unwrap();
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
        jpeg.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        jpeg.extend_from_slice(exif);
        jpeg.extend_from_slice(&encoded.get_ref()[2..]);
        let path =
            std::env::temp_dir().join(format!("clipstash-preview-exif-{}.jpg", std::process::id()));
        fs::write(&path, &jpeg).unwrap();
        assert_eq!(dimensions(&path).unwrap(), (40, 20));
        assert_eq!(fs::read(&path).unwrap(), jpeg);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn preview_upload_reads_dimensions_and_only_current_lease_can_delete() {
        let root =
            std::env::temp_dir().join(format!("clipstash-preview-test-{}", std::process::id()));
        let mut bytes = Cursor::new(Vec::new());
        image::RgbaImage::new(20, 40)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let first = save_upload(&root, bytes.get_ref()).unwrap();
        assert_eq!((first.width, first.height), (20, 40));
        let second = save_upload(&root, bytes.get_ref()).unwrap();
        release_preview_upload(first.lease.unwrap()).unwrap();
        assert!(Path::new(&second.path).exists());
        assert!(save_upload(&root, b"not an image").is_err());
        assert!(Path::new(&second.path).exists());
        release_preview_upload(second.lease.unwrap()).unwrap();
        assert!(!Path::new(&second.path).exists());
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn grant_file_scope_skips_allow_when_already_allowed_and_propagates_errors() {
        let mut allow_calls = 0;
        grant_file_scope(true, || {
            allow_calls += 1;
            Err("must not run".into())
        })
        .expect("already allowed must succeed without extending the scope");
        assert_eq!(allow_calls, 0, "already allowed must not call allow");

        grant_file_scope(false, || {
            allow_calls += 1;
            Ok(())
        })
        .expect("out-of-scope file must be granted");
        assert_eq!(allow_calls, 1);

        let err = grant_file_scope(false, || Err("grant failed".into()))
            .expect_err("allow failure must propagate");
        assert_eq!(err, "grant failed");
    }
}
