use crate::legacy_data::LegacyMessageImage;
use chrono::Local;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

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

    Ok(LegacyDbBackup {
        source_path: path_to_string(db_path.to_path_buf()),
        backup_path: path_to_string(backup_path),
        bytes_copied,
    })
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
        backup_dir: path_to_string(backup_dir),
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

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}
