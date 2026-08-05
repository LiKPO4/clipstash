use crate::{legacy_model::LegacyMessageImage, legacy_paths::path_to_string};
use chrono::Local;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_DB_BACKUP_KEEP: usize = 10;
const DB_BACKUP_FILE_PREFIX: &str = "clipstash.db.bak-";

#[derive(Serialize)]
pub struct LegacyDbBackup {
    pub source_path: String,
    pub backup_path: String,
    pub bytes_copied: u64,
}

#[derive(Serialize)]
pub struct LegacyImageFilesBackup {
    pub backup_dir: String,
    pub filenames: Vec<String>,
}

pub(crate) fn create_legacy_db_backup_for_path(db_path: &Path) -> Result<LegacyDbBackup, String> {
    if !db_path.is_file() {
        return Err(format!("备份失败，数据库不存在：{}", db_path.display()));
    }

    let parent = db_path
        .parent()
        .ok_or_else(|| format!("备份失败，无法定位数据库目录：{}", db_path.display()))?;
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup_path = next_backup_path(parent, &timestamp.to_string());
    let bytes_copied =
        fs::copy(db_path, &backup_path).map_err(|err| format!("备份旧数据库失败：{err}"))?;
    // 保留策略：清理失败只忽略，不阻塞备份与写入
    let _ = prune_legacy_db_backups(parent);

    Ok(LegacyDbBackup {
        source_path: path_to_string(db_path),
        backup_path: path_to_string(&backup_path),
        bytes_copied,
    })
}

/// 只保留最近 [MAX_DB_BACKUP_KEEP] 份数据库备份，按文件名时间戳排序删除更旧的。
fn prune_legacy_db_backups(db_dir: &Path) -> Result<usize, String> {
    let mut backups: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(db_dir)
        .map_err(|err| format!("读取备份目录失败：{}：{err}", db_dir.display()))?
    {
        let entry = entry.map_err(|err| format!("读取备份目录条目失败：{err}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with(DB_BACKUP_FILE_PREFIX) {
            backups.push((file_name, entry.path()));
        }
    }

    backups.sort_by(|left, right| left.0.cmp(&right.0));
    let overflow = backups.len().saturating_sub(MAX_DB_BACKUP_KEEP);
    let mut removed = 0;
    for (_, path) in backups.into_iter().take(overflow) {
        fs::remove_file(&path)
            .map_err(|err| format!("删除旧备份失败：{}：{err}", path.display()))?;
        removed += 1;
    }
    Ok(removed)
}

pub(crate) fn backup_message_image_files(
    data_dir: &Path,
    images: &[LegacyMessageImage],
) -> Result<Option<LegacyImageFilesBackup>, String> {
    let images_dir = data_dir.join("images");
    let existing_images: Vec<&LegacyMessageImage> = images
        .iter()
        .filter(|image| images_dir.join(&image.filename).is_file())
        .collect();
    if existing_images.is_empty() {
        return Ok(None);
    }

    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup_dir = next_image_backup_dir(data_dir, &timestamp.to_string());
    fs::create_dir_all(&backup_dir).map_err(|err| format!("创建旧图片备份目录失败：{err}"))?;

    let mut filenames = Vec::new();
    for image in existing_images {
        let source = images_dir.join(&image.filename);
        let target = backup_dir.join(&image.filename);
        fs::copy(&source, &target)
            .map_err(|err| format!("备份旧图片文件失败：{}：{err}", source.display()))?;
        filenames.push(image.filename.clone());
    }

    Ok(Some(LegacyImageFilesBackup {
        backup_dir: path_to_string(&backup_dir),
        filenames,
    }))
}

pub(crate) fn next_backup_path(parent: &Path, timestamp: &str) -> PathBuf {
    let first = parent.join(format!("clipstash.db.bak-{timestamp}"));
    if !first.exists() {
        return first;
    }

    for index in 1.. {
        let candidate = parent.join(format!("clipstash.db.bak-{timestamp}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("backup suffix search is unbounded");
}

fn next_image_backup_dir(data_dir: &Path, timestamp: &str) -> PathBuf {
    let first = data_dir.join(format!("images.bak-{timestamp}"));
    if !first.exists() {
        return first;
    }

    for index in 1.. {
        let candidate = data_dir.join(format!("images.bak-{timestamp}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("image backup suffix search is unbounded");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, process};

    #[test]
    fn keeps_only_most_recent_ten_db_backups_after_twelve_creates() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-backup-retention-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create retention fixture dir");

        let db_path = data_dir.join("clipstash.db");
        fs::write(&db_path, b"legacy-db-bytes").expect("write retention fixture db");

        for _ in 0..12 {
            create_legacy_db_backup_for_path(&db_path).expect("create db backup");
        }

        let backup_count = fs::read_dir(&data_dir)
            .expect("read retention fixture dir")
            .filter_map(|entry| {
                let name = entry.expect("read retention fixture entry");
                let name = name.file_name().to_string_lossy().to_string();
                name.starts_with(DB_BACKUP_FILE_PREFIX).then_some(name)
            })
            .count();
        assert_eq!(backup_count, 10);
        assert!(db_path.is_file());

        fs::remove_dir_all(data_dir).expect("remove retention fixture");
    }
}
