use crate::{
    legacy_paths::{legacy_data_dir, path_to_string},
    legacy_query::{query_count, read_legacy_stats_from_dir, LegacyStats},
    legacy_schema::ensure_legacy_schema,
};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::{fs, path::Path, time::SystemTime};

const RECENT_BACKUP_LIMIT: usize = 8;

#[derive(Serialize)]
pub struct LegacyBackupFileInfo {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified_at: Option<String>,
}

#[derive(Serialize)]
pub struct LegacySafetyReport {
    pub stats: LegacyStats,
    pub joined_image_count: i64,
    pub orphan_image_count: i64,
    pub db_backup_count: usize,
    pub image_backup_count: usize,
    pub recent_db_backups: Vec<LegacyBackupFileInfo>,
    pub recent_image_backups: Vec<LegacyBackupFileInfo>,
}

pub fn read_legacy_safety_report() -> Result<LegacySafetyReport, String> {
    let data_dir = legacy_data_dir()?;
    let db_path = data_dir.join("clipstash.db");
    let stats = read_legacy_stats_from_dir(data_dir.clone())?;
    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("只读打开旧数据库准备安全审计失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let joined_image_count = query_count(
        &conn,
        "SELECT COUNT(*) \
         FROM message_images mi \
         JOIN messages m ON m.id = mi.message_id",
    )?;
    let orphan_image_count = query_count(
        &conn,
        "SELECT COUNT(*) \
         FROM message_images mi \
         LEFT JOIN messages m ON m.id = mi.message_id \
         WHERE m.id IS NULL",
    )?;

    let (db_backup_count, recent_db_backups) = list_recent_backups(&data_dir, "clipstash.db.bak-")?;
    let (image_backup_count, recent_image_backups) = list_recent_backups(&data_dir, "images.bak-")?;

    Ok(LegacySafetyReport {
        stats,
        joined_image_count,
        orphan_image_count,
        db_backup_count,
        image_backup_count,
        recent_db_backups,
        recent_image_backups,
    })
}

fn list_recent_backups(
    data_dir: &Path,
    prefix: &str,
) -> Result<(usize, Vec<LegacyBackupFileInfo>), String> {
    if !data_dir.is_dir() {
        return Ok((0, Vec::new()));
    }

    let mut backups = Vec::new();
    for entry in fs::read_dir(data_dir).map_err(|err| format!("读取旧数据目录失败：{err}"))?
    {
        let entry = entry.map_err(|err| format!("读取旧数据目录项失败：{err}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(prefix) {
            continue;
        }

        let metadata = entry
            .metadata()
            .map_err(|err| format!("读取备份文件信息失败：{}：{err}", path.display()))?;
        backups.push(BackupCandidate {
            info: LegacyBackupFileInfo {
                name,
                path: path_to_string(&path),
                bytes: metadata.len(),
                modified_at: metadata.modified().ok().map(format_system_time),
            },
            modified: metadata.modified().ok(),
        });
    }

    let total = backups.len();
    backups.sort_by(|left, right| right.modified.cmp(&left.modified));
    let recent = backups
        .into_iter()
        .take(RECENT_BACKUP_LIMIT)
        .map(|candidate| candidate.info)
        .collect();

    Ok((total, recent))
}

struct BackupCandidate {
    info: LegacyBackupFileInfo,
    modified: Option<SystemTime>,
}

fn format_system_time(value: SystemTime) -> String {
    match value.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}
