use crate::{app_data, legacy_write_exec::create_message_from_image_files_for_path};
use serde::Deserialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::Manager;

static IMPORT_LOCK: Mutex<()> = Mutex::new(());
#[derive(Deserialize)]
struct ShareManifest {
    text: String,
    images: Vec<String>,
}

fn packet_dir(cache_root: &Path, share_id: &str) -> Result<PathBuf, String> {
    if share_id.len() != 36 || !share_id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err("非法分享 ID".into());
    }
    let cache_root = cache_root.canonicalize().map_err(|e| e.to_string())?;
    let staging_root = cache_root.join("clipstash-shares");
    if fs::symlink_metadata(&staging_root)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("分享缓存不能是链接".into());
    }
    let root = staging_root.canonicalize().map_err(|e| e.to_string())?;
    let candidate = root.join(share_id);
    if fs::symlink_metadata(&candidate)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("分享目录不能是链接".into());
    }
    let resolved = candidate.canonicalize().map_err(|e| e.to_string())?;
    if resolved.parent() != Some(root.as_path()) {
        return Err("分享目录越界".into());
    }
    Ok(resolved)
}

fn read_manifest(dir: &Path) -> Result<(String, Vec<PathBuf>), String> {
    let manifest_path = dir.join("manifest.json");
    if fs::symlink_metadata(&manifest_path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("分享清单不能是链接".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(manifest_path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("分享清单过大".into());
    }
    let manifest: ShareManifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    let mut files = Vec::new();
    for name in manifest.images {
        if !name
            .strip_suffix(".bin")
            .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|c| c.is_ascii_digit()))
        {
            return Err("非法分享图片文件名".into());
        }
        let path = dir.join(name);
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() == 0
            || meta.len() > 256 * 1024 * 1024
        {
            return Err("分享图片无效或超过 256MiB".into());
        }
        total += meta.len();
        if total > 4 * 1024 * 1024 * 1024 {
            return Err("分享图片合计超过 4GiB".into());
        }
        files.push(path);
    }
    let text = manifest.text.trim().to_string();
    if text.is_empty() && files.is_empty() {
        return Err("分享内容为空".into());
    }
    Ok((text, files))
}

#[tauri::command(async)]
pub fn import_android_share(
    app: tauri::AppHandle,
    share_id: String,
) -> Result<crate::legacy_model::LegacyMessage, String> {
    let _guard = IMPORT_LOCK.lock().map_err(|_| "分享导入锁已损坏")?;
    let dir = packet_dir(
        &app.path().app_cache_dir().map_err(|e| e.to_string())?,
        &share_id,
    )?;
    let result = (|| {
        let (text, files) = read_manifest(&dir)?;
        let data_dir = app_data::ready_app_data_dir_path()?;
        create_message_from_image_files_for_path(
            &data_dir.join("clipstash.db"),
            (!text.is_empty()).then_some(text),
            files,
        )
    })();
    // Only this validated, app-owned packet is removed; user source URIs and originals are untouched.
    let _ = fs::remove_dir_all(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequential_file_reads_rollback_db_and_saved_images_on_later_failure() {
        let root =
            std::env::temp_dir().join(format!("clipstash-share-write-test-{}", std::process::id()));
        fs::create_dir_all(root.join("images")).unwrap();
        let db = root.join("clipstash.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE messages (id INTEGER PRIMARY KEY, text_content TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP, archived INTEGER DEFAULT 0, archived_at TEXT); CREATE TABLE message_images (id INTEGER PRIMARY KEY, message_id INTEGER, image_filename TEXT);").unwrap();
        let png = root.join("0.bin");
        let jpeg = root.join("1.bin");
        fs::write(&png, b"\x89PNG\r\n\x1a\nfixture").unwrap();
        fs::write(&jpeg, b"\xff\xd8\xfffixture").unwrap();
        assert!(create_message_from_image_files_for_path(
            &db,
            Some("text".into()),
            vec![png.clone(), root.join("missing.bin")]
        )
        .is_err());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(fs::read_dir(root.join("images")).unwrap().count(), 0);
        let message = create_message_from_image_files_for_path(
            &db,
            Some("text".into()),
            vec![jpeg.clone(), png.clone()],
        )
        .unwrap();
        assert_eq!(message.images.len(), 2);
        assert!(message.images[0].filename.ends_with(".jpg"));
        assert!(message.images[1].filename.ends_with(".png"));
        assert_eq!(
            fs::read(&message.images[0].path).unwrap(),
            fs::read(jpeg).unwrap()
        );
        assert_eq!(
            fs::read(&message.images[1].path).unwrap(),
            fs::read(png).unwrap()
        );
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn share_packet_rejects_traversal_and_reads_ordered_files() {
        let root =
            std::env::temp_dir().join(format!("clipstash-share-test-{}", std::process::id()));
        let id = "01234567-89ab-cdef-0123-456789abcdef";
        let dir = root.join("clipstash-shares").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("0.bin"), b"one").unwrap();
        fs::write(dir.join("1.bin"), b"two").unwrap();
        fs::write(
            dir.join("manifest.json"),
            r#"{"text":" text ","images":["1.bin","0.bin"]}"#,
        )
        .unwrap();
        assert!(packet_dir(&root, "../outside").is_err());
        let validated = packet_dir(&root, id).unwrap();
        let (text, files) = read_manifest(&validated).unwrap();
        assert_eq!(text, "text");
        assert_eq!(fs::read(&files[0]).unwrap(), b"two");
        fs::write(
            dir.join("manifest.json"),
            r#"{"text":"","images":["../outside"]}"#,
        )
        .unwrap();
        assert!(read_manifest(&validated).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
