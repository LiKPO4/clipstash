use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::Manager;

const CHUNK_BYTES: usize = 1024 * 1024;
static UPLOADS: OnceLock<Mutex<HashMap<String, Upload>>> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static ACTIVE_PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);
fn active_paths() -> &'static Mutex<HashSet<PathBuf>> {
    ACTIVE_PATHS.get_or_init(|| Mutex::new(HashSet::new()))
}
fn uploads() -> &'static Mutex<HashMap<String, Upload>> {
    UPLOADS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct Upload {
    path: PathBuf,
    file: Option<File>,
    expected: u64,
    written: u64,
    last_activity: Instant,
}
impl Drop for Upload {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
        active_paths()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.path);
    }
}
impl Upload {
    fn create(root: &Path, expected: u64) -> Result<(String, Self), String> {
        if expected == 0 {
            return Err("导入数据包为空".into());
        }
        fs::create_dir_all(root).map_err(|err| format!("创建导入暂存目录失败：{err}"))?;
        if fs::symlink_metadata(root)
            .map_err(|err| err.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("导入暂存目录不能是符号链接".into());
        }
        let root = root.canonicalize().map_err(|err| err.to_string())?;
        let id = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let path = root.join(format!("upload-{id}.zip"));
        let file = File::options()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|err| format!("创建导入暂存文件失败：{err}"))?;
        active_paths()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(path.clone());
        Ok((
            id,
            Self {
                path,
                file: Some(file),
                expected,
                written: 0,
                last_activity: Instant::now(),
            },
        ))
    }
    fn append(&mut self, offset: u64, bytes: &[u8]) -> Result<u64, String> {
        if offset != self.written || bytes.is_empty() || bytes.len() > CHUNK_BYTES {
            return Err("导入分块顺序或大小不正确".into());
        }
        let next = self
            .written
            .checked_add(bytes.len() as u64)
            .filter(|next| *next <= self.expected)
            .ok_or("导入数据超过声明大小")?;
        self.file
            .as_mut()
            .ok_or("导入暂存已经关闭")?
            .write_all(bytes)
            .map_err(|err| format!("写入导入分块失败：{err}"))?;
        self.written = next;
        self.last_activity = Instant::now();
        Ok(next)
    }
    fn finish(&mut self) -> Result<(), String> {
        if self.written != self.expected {
            return Err("导入数据包尚未传输完整".into());
        }
        let mut file = self.file.take().ok_or("导入暂存已经关闭")?;
        file.flush()
            .map_err(|err| format!("保存导入暂存失败：{err}"))?;
        drop(file);
        Ok(())
    }
}

// Run only when beginning a new upload. In-flight imports keep their path pinned
// even after leaving UPLOADS, so a long import is never mistaken for an orphan.
fn cleanup_stale_uploads(root: &Path) {
    cleanup_stale_uploads_at(root, Instant::now());
}

fn cleanup_stale_uploads_at(root: &Path, now: Instant) {
    if let Ok(mut registry) = uploads().lock() {
        registry
            .retain(|_, upload| now.saturating_duration_since(upload.last_activity) < RETENTION);
    }
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let pinned = active_paths()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = name
            .strip_prefix("upload-")
            .and_then(|name| name.strip_suffix(".zip"))
        else {
            continue;
        };
        let parts: Vec<_> = id.split('-').collect();
        if parts.len() != 3
            || parts[0].parse::<u32>().is_err()
            || parts[1].parse::<u128>().is_err()
            || parts[2].parse::<u64>().is_err()
        {
            continue;
        }
        let path = entry.path();
        if pinned.contains(&path) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata
                .modified()
                .ok()
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age >= RETENTION)
        {
            let _ = fs::remove_file(path);
        }
    }
}

#[tauri::command(async)]
pub fn begin_zip_upload(
    app: tauri::AppHandle,
    filename: String,
    size: u64,
) -> Result<String, String> {
    if !Path::new(&filename)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err("导入数据包必须是 .zip 文件".into());
    }
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|err| err.to_string())?
        .join("zip-uploads");
    let (id, upload) = Upload::create(&root, size)?;
    cleanup_stale_uploads(upload.path.parent().expect("upload has parent"));
    uploads()
        .lock()
        .map_err(|_| "导入暂存锁异常")?
        .insert(id.clone(), upload);
    Ok(id)
}

#[tauri::command(async)]
pub fn append_zip_upload(request: tauri::ipc::Request<'_>) -> Result<u64, String> {
    let id = request
        .headers()
        .get("x-upload-id")
        .and_then(|value| value.to_str().ok())
        .ok_or("缺少导入暂存标识")?;
    let offset = request
        .headers()
        .get("x-upload-offset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or("缺少导入分块偏移")?;
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes,
        _ => return Err("导入分块必须是二进制数据".into()),
    };
    let mut registry = uploads().lock().map_err(|_| "导入暂存锁异常")?;
    let mut upload = registry.remove(id).ok_or("导入暂存不存在或已结束")?;
    let next = upload.append(offset, bytes)?;
    registry.insert(id.to_string(), upload);
    Ok(next)
}

#[tauri::command(async)]
pub fn abort_zip_upload(upload_id: String) -> Result<(), String> {
    uploads()
        .lock()
        .map_err(|_| "导入暂存锁异常")?
        .remove(&upload_id);
    Ok(())
}

#[tauri::command(async)]
pub fn finish_zip_upload(
    upload_id: String,
    webview: tauri::Webview,
    progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<crate::data_transfer::DataImportResult, String> {
    let mut upload = uploads()
        .lock()
        .map_err(|_| "导入暂存锁异常")?
        .remove(&upload_id)
        .ok_or("导入暂存不存在或已结束")?;
    upload.finish()?;
    crate::transfer_progress::run(progress.map(|id| id.channel_on(webview)), || {
        crate::data_transfer::import_data_zip_from_path(upload.path.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_active_imports_recent_files_and_unowned_entries() {
        let root =
            std::env::temp_dir().join(format!("clipstash-upload-cleanup-{}", std::process::id()));
        let (_, mut importing) = Upload::create(&root, 1).unwrap();
        importing.append(0, b"x").unwrap();
        importing.finish().unwrap(); // No registry entry: the import owns this lease.
        let old = std::time::SystemTime::now() - RETENTION - Duration::from_secs(60);
        File::options()
            .write(true)
            .open(&importing.path)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let stale = root.join("upload-1-2-3.zip");
        let recent = root.join("upload-1-2-4.zip");
        let unrelated = root.join("notes.zip");
        let malformed = root.join("upload-user-notes.zip");
        for path in [&stale, &recent, &unrelated, &malformed] {
            fs::write(path, b"fixture").unwrap();
        }
        for path in [&stale, &unrelated, &malformed] {
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        let (id, mut abandoned) = Upload::create(&root, 2).unwrap();
        abandoned.last_activity = Instant::now();
        let abandoned_path = abandoned.path.clone();
        uploads().lock().unwrap().insert(id, abandoned);
        cleanup_stale_uploads_at(&root, Instant::now() + RETENTION + Duration::from_secs(1));
        assert!(!stale.exists());
        assert!(!abandoned_path.exists());
        for path in [&importing.path, &recent, &unrelated, &malformed] {
            assert!(path.exists());
        }
        drop(importing);
        for path in [recent, unrelated, malformed] {
            fs::remove_file(path).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn chunks_are_ordered_bounded_and_removed_after_finish_or_error() {
        let root =
            std::env::temp_dir().join(format!("clipstash-zip-chunks-{}", std::process::id()));
        let (_, mut upload) = Upload::create(&root, CHUNK_BYTES as u64 + 3).unwrap();
        let path = upload.path.clone();
        assert!(upload.append(1, b"abc").is_err());
        assert!(upload.append(0, &vec![0; CHUNK_BYTES + 1]).is_err());
        assert!(upload.finish().is_err());
        assert_eq!(
            upload.append(0, &vec![7; CHUNK_BYTES]).unwrap(),
            CHUNK_BYTES as u64
        );
        assert!(upload.append(CHUNK_BYTES as u64, b"abcd").is_err());
        upload.append(CHUNK_BYTES as u64, b"end").unwrap();
        upload.finish().unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), CHUNK_BYTES as u64 + 3);
        assert!(fs::read(&path).unwrap().ends_with(b"end"));
        drop(upload);
        assert!(!path.exists());
        let (_, mut failed) = Upload::create(&root, 4).unwrap();
        let path = failed.path.clone();
        failed.append(0, b"ab").unwrap();
        assert!(failed.finish().is_err());
        drop(failed);
        assert!(!path.exists());
        fs::remove_dir(root).unwrap();
    }
}
