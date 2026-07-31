use crate::legacy_model::LegacyMessageImage;
use chrono::Local;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn remove_old_message_image_files(images: &[LegacyMessageImage]) {
    for image in images {
        let _ = fs::remove_file(&image.path);
    }
}

pub(crate) fn save_image_file(path: &Path, image_data: &[u8]) -> Result<(), String> {
    fs::write(path, image_data).map_err(|err| format!("保存图片文件失败：{err}"))
}

/// 按文件内容魔数推断图片扩展名，避免把 JPEG 等字节错误落盘为 .png。
/// 无法识别时回退 "png"，保持桌面剪贴板 RGBA→PNG 写入路径的原有行为。
pub(crate) fn sniff_image_extension(image_data: &[u8]) -> &'static str {
    if image_data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if image_data.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if image_data.starts_with(b"GIF87a") || image_data.starts_with(b"GIF89a") {
        "gif"
    } else if image_data.starts_with(b"BM") {
        "bmp"
    } else if image_data.len() >= 12
        && &image_data[0..4] == b"RIFF"
        && &image_data[8..12] == b"WEBP"
    {
        "webp"
    } else {
        "png"
    }
}

pub(crate) fn next_image_filename(images_dir: &Path, index: usize, extension: &str) -> String {
    let timestamp = Local::now().format("%Y%m%d%H%M%S%3f");
    let process_id = std::process::id();

    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let filename =
            format!("clipstash-next-{timestamp}-{process_id}-{index}{suffix}.{extension}");
        if !images_dir.join(&filename).exists() {
            return filename;
        }
    }

    unreachable!("image filename suffix search is unbounded");
}

pub(crate) fn resolve_legacy_image_path(
    data_dir: &Path,
    filename: &str,
) -> Result<PathBuf, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return Err("复制图片失败，图片文件名不能为空".to_string());
    }
    let candidate_name = Path::new(trimmed);
    if candidate_name.components().count() != 1 {
        return Err(format!("复制图片失败，非法图片文件名：{trimmed}"));
    }

    let image_path = data_dir.join("images").join(trimmed);
    if !image_path.is_file() {
        return Err(format!(
            "复制图片失败，图片文件不存在：{}",
            image_path.display()
        ));
    }

    Ok(image_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_image_extension_detects_common_formats() {
        assert_eq!(sniff_image_extension(b"\x89PNG\r\n\x1a\nrest"), "png");
        assert_eq!(sniff_image_extension(b"\xff\xd8\xff\xe0JFIF"), "jpg");
        assert_eq!(sniff_image_extension(b"GIF89a..."), "gif");
        assert_eq!(sniff_image_extension(b"GIF87a..."), "gif");
        assert_eq!(sniff_image_extension(b"BM...."), "bmp");
        assert_eq!(
            sniff_image_extension(b"RIFF\x04\x00\x00\x00WEBPVP8 "),
            "webp"
        );
    }

    #[test]
    fn sniff_image_extension_falls_back_to_png_for_unknown_bytes() {
        assert_eq!(sniff_image_extension(b"image-one"), "png");
        assert_eq!(sniff_image_extension(b""), "png");
    }

    #[test]
    fn next_image_filename_uses_given_extension() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("clipstash-next-image-files-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();

        let filename = next_image_filename(&dir, 0, "jpg");
        assert!(filename.starts_with("clipstash-next-"));
        assert!(filename.ends_with(".jpg"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
