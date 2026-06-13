use crate::{
    legacy_clipboard::{
        copy_legacy_image_to_clipboard_from_dir,
        copy_legacy_message_import_queue_item_to_clipboard_from_dir,
        copy_legacy_message_text_to_clipboard_from_dir,
        preview_legacy_message_import_queue_from_dir,
        stage_legacy_message_import_to_clipboard_from_dir, LegacyCopyImageResult,
        LegacyCopyTextResult, LegacyImportQueueCopyResult, LegacyImportQueuePreview,
        LegacyImportStageResult,
    },
    legacy_data::{
        LegacyArchiveMessageResult, LegacyCreateTextMessageResult, LegacyDbBackup,
        LegacyDeleteMessageResult, LegacyImageFilesBackup, LegacyReplaceImagesResult,
        LegacyUpdateMessageResult, LegacyWriteAudit,
    },
    legacy_image_files::resolve_legacy_image_path,
    legacy_model::{LegacyMessage, LegacyMessagePage, MessageView, SortOrder},
    legacy_paths::{legacy_data_dir, path_to_string},
    legacy_query::{list_legacy_messages_from_dir, query_count, read_legacy_stats_from_dir},
    legacy_schema::ensure_legacy_schema,
    legacy_write_exec::{
        create_image_message_for_path, create_mixed_message_for_path, create_text_message_for_path,
        delete_message_for_path, replace_message_images_for_path, set_message_archived_for_path,
        update_text_message_for_path,
    },
    legacy_write_validation::{normalize_optional_text_message, normalize_text_message},
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const APP_DATA_DIR_NAME: &str = "ClipStash Next";
const APP_DB_NAME: &str = "clipstash.db";
const DATA_LOCATION_FILE_NAME: &str = "data-location.json";

#[derive(Serialize)]
pub struct AppStats {
    pub data_dir: String,
    pub db_path: String,
    pub images_dir: String,
    pub db_exists: bool,
    pub images_dir_exists: bool,
    pub normal_count: i64,
    pub archived_count: i64,
    pub total_count: i64,
}

#[derive(Serialize)]
pub struct AppMigrationResult {
    pub inserted_messages: i64,
    pub skipped_messages: i64,
    pub copied_images: i64,
    pub skipped_images: i64,
    pub legacy_message_count: i64,
    pub legacy_image_count: i64,
    pub stats: AppStats,
}

#[derive(Serialize)]
pub struct AppDataMoveResult {
    pub previous_data_dir: String,
    pub data_dir: String,
    pub stats: AppStats,
}

#[derive(Serialize)]
pub struct AppDataRepairResult {
    pub copied_db: bool,
    pub copied_images: i64,
    pub skipped_images: i64,
    pub source_data_dir: String,
    pub stats: AppStats,
}

#[derive(Deserialize, Serialize)]
struct DataLocationConfig {
    data_dir: String,
}

struct AppPaths {
    data_dir: PathBuf,
    db_path: PathBuf,
    images_dir: PathBuf,
}

pub fn app_data_dir_path() -> Result<PathBuf, String> {
    Ok(app_paths()?.data_dir)
}

pub fn default_app_data_dir_path() -> Result<PathBuf, String> {
    Ok(appdata_base_dir()?.join(APP_DATA_DIR_NAME))
}

pub fn ensure_app_data_ready() -> Result<AppStats, String> {
    let paths = app_paths()?;
    fs::create_dir_all(&paths.images_dir).map_err(|err| format!("创建应用图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(&paths.db_path).map_err(|err| format!("打开应用数据库失败：{err}"))?;
    ensure_app_schema(&conn)?;

    if !has_migration_state(&conn)? {
        migrate_legacy_once(&mut conn, &paths)?;
    }

    read_app_stats_from_paths(&paths)
}

pub fn read_app_stats() -> Result<AppStats, String> {
    ensure_app_data_ready()
}

pub fn migrate_legacy_data() -> Result<AppMigrationResult, String> {
    let paths = app_paths()?;
    fs::create_dir_all(&paths.images_dir).map_err(|err| format!("创建应用图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(&paths.db_path).map_err(|err| format!("打开应用数据库失败：{err}"))?;
    ensure_app_schema(&conn)?;
    merge_legacy_data(&mut conn, &paths)
}

pub fn move_app_data_to_dir(target_dir: PathBuf) -> Result<AppDataMoveResult, String> {
    ensure_app_data_ready()?;
    let current_paths = app_paths()?;
    let (current_dir, target_dir) = move_app_data_files(
        &current_paths.data_dir,
        target_dir,
        &default_app_data_dir_path()?,
    )?;
    let stats = read_app_stats()?;

    Ok(AppDataMoveResult {
        previous_data_dir: path_to_string(&current_dir),
        data_dir: path_to_string(&target_dir),
        stats,
    })
}

pub fn repair_app_data_dir() -> Result<AppDataRepairResult, String> {
    let paths = app_paths()?;
    let default_dir = default_app_data_dir_path()?;
    repair_app_data_dir_from_default(&paths, &default_dir)
}

fn repair_app_data_dir_from_default(
    paths: &AppPaths,
    default_dir: &Path,
) -> Result<AppDataRepairResult, String> {
    let current_dir = normalize_existing_or_create_dir(&paths.data_dir)?;
    let source_dir = normalize_existing_or_create_dir(&default_dir)?;

    if current_dir == source_dir {
        return Ok(AppDataRepairResult {
            copied_db: false,
            copied_images: 0,
            skipped_images: 0,
            source_data_dir: path_to_string(&source_dir),
            stats: read_app_stats_from_paths(paths)?,
        });
    }

    let mut copied_db = false;
    let source_db = source_dir.join(APP_DB_NAME);
    if source_db.is_file() && !paths.db_path.is_file() {
        fs::copy(&source_db, &paths.db_path).map_err(|err| {
            format!(
                "修复数据库失败：{} -> {}：{err}",
                source_db.display(),
                paths.db_path.display()
            )
        })?;
        copied_db = true;
    }

    let mut copied_images = 0_i64;
    let mut skipped_images = 0_i64;
    let source_images_dir = source_dir.join("images");
    fs::create_dir_all(&paths.images_dir).map_err(|err| format!("创建图片目录失败：{err}"))?;
    if source_images_dir.is_dir() {
        for entry in fs::read_dir(&source_images_dir).map_err(|err| {
            format!(
                "读取修复来源图片目录失败：{}：{err}",
                source_images_dir.display()
            )
        })? {
            let entry = entry.map_err(|err| format!("读取修复来源图片失败：{err}"))?;
            let source_path = entry.path();
            let file_type = entry.file_type().map_err(|err| {
                format!("读取修复来源文件类型失败：{}：{err}", source_path.display())
            })?;
            if !file_type.is_file() {
                continue;
            }
            let target_path = paths.images_dir.join(entry.file_name());
            if target_path.exists() {
                skipped_images += 1;
                continue;
            }
            fs::copy(&source_path, &target_path).map_err(|err| {
                format!(
                    "修复图片失败：{} -> {}：{err}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            copied_images += 1;
        }
    }

    Ok(AppDataRepairResult {
        copied_db,
        copied_images,
        skipped_images,
        source_data_dir: path_to_string(&source_dir),
        stats: read_app_stats_from_paths(paths)?,
    })
}

pub fn list_messages(
    view: MessageView,
    sort: SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LegacyMessagePage, String> {
    let paths = ready_paths()?;
    list_legacy_messages_from_dir(paths.data_dir, view, sort, offset, limit)
}

pub fn create_text_message(text_content: String) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized = normalize_text_message(text_content)?;
    let paths = ready_paths()?;
    let message = create_text_message_for_path(&paths.db_path, Some(normalized))?;
    Ok(write_result("create_text_message", message))
}

pub fn create_image_message(
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let paths = ready_paths()?;
    let message = create_image_message_for_path(&paths.db_path, images_data)?;
    Ok(write_result("create_image_message", message))
}

pub fn create_mixed_message(
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized = normalize_text_message(text_content)?;
    let paths = ready_paths()?;
    let message = create_mixed_message_for_path(&paths.db_path, Some(normalized), images_data)?;
    Ok(write_result("create_mixed_message", message))
}

pub fn update_message_text(
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyUpdateMessageResult, String> {
    let normalized = normalize_optional_text_message(text_content);
    let paths = ready_paths()?;
    let message = update_text_message_for_path(&paths.db_path, message_id, normalized)?;
    Ok(LegacyUpdateMessageResult {
        backup: empty_backup(&paths.db_path),
        audit: audit("update_message_text", message.id),
        message,
    })
}

pub fn replace_message_images(
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyReplaceImagesResult, String> {
    let paths = ready_paths()?;
    let message = replace_message_images_for_path(&paths.db_path, message_id, images_data)?;
    Ok(LegacyReplaceImagesResult {
        backup: empty_backup(&paths.db_path),
        audit: audit("replace_message_images", message.id),
        image_backup: None::<LegacyImageFilesBackup>,
        message,
    })
}

pub fn delete_message(message_id: i64) -> Result<LegacyDeleteMessageResult, String> {
    let paths = ready_paths()?;
    let message = delete_message_for_path(&paths.db_path, message_id)?;
    Ok(LegacyDeleteMessageResult {
        backup: empty_backup(&paths.db_path),
        audit: audit("delete_message", message.id),
        image_backup: None::<LegacyImageFilesBackup>,
        message,
    })
}

pub fn set_message_archived(
    message_id: i64,
    archived: bool,
) -> Result<LegacyArchiveMessageResult, String> {
    let paths = ready_paths()?;
    let message = set_message_archived_for_path(&paths.db_path, message_id, archived)?;
    Ok(LegacyArchiveMessageResult {
        backup: empty_backup(&paths.db_path),
        audit: audit("set_message_archived", message.id),
        message,
    })
}

pub fn copy_image_to_clipboard(filename: String) -> Result<LegacyCopyImageResult, String> {
    let paths = ready_paths()?;
    copy_legacy_image_to_clipboard_from_dir(paths.data_dir, filename)
}

pub fn read_image_bytes(filename: String) -> Result<Vec<u8>, String> {
    let paths = ready_paths()?;
    let image_path = resolve_legacy_image_path(&paths.data_dir, &filename)?;
    fs::read(&image_path)
        .map_err(|err| format!("读取图片文件失败：{}：{err}", image_path.display()))
}

pub fn copy_message_text_to_clipboard(message_id: i64) -> Result<LegacyCopyTextResult, String> {
    let paths = ready_paths()?;
    copy_legacy_message_text_to_clipboard_from_dir(paths.data_dir, message_id)
}

pub fn stage_message_import_to_clipboard(
    message_id: i64,
) -> Result<LegacyImportStageResult, String> {
    let paths = ready_paths()?;
    stage_legacy_message_import_to_clipboard_from_dir(paths.data_dir, message_id)
}

pub fn preview_message_import_queue(message_id: i64) -> Result<LegacyImportQueuePreview, String> {
    let paths = ready_paths()?;
    preview_legacy_message_import_queue_from_dir(paths.data_dir, message_id)
}

pub fn copy_message_import_queue_item_to_clipboard(
    message_id: i64,
    item_index: usize,
) -> Result<LegacyImportQueueCopyResult, String> {
    let paths = ready_paths()?;
    copy_legacy_message_import_queue_item_to_clipboard_from_dir(
        paths.data_dir,
        message_id,
        item_index,
    )
}

fn ready_paths() -> Result<AppPaths, String> {
    ensure_app_data_ready()?;
    app_paths()
}

fn app_paths() -> Result<AppPaths, String> {
    let data_dir = configured_app_data_dir()?;
    Ok(AppPaths {
        db_path: data_dir.join(APP_DB_NAME),
        images_dir: data_dir.join("images"),
        data_dir,
    })
}

fn appdata_base_dir() -> Result<PathBuf, String> {
    env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .ok_or_else(|| "无法定位 Windows 应用数据目录".to_string())
}

fn configured_app_data_dir() -> Result<PathBuf, String> {
    let default_dir = default_app_data_dir_path()?;
    let location_path = default_dir.join(DATA_LOCATION_FILE_NAME);
    let Ok(text) = fs::read_to_string(&location_path) else {
        return Ok(default_dir);
    };
    let config = serde_json::from_str::<DataLocationConfig>(&text)
        .map_err(|err| format!("解析数据目录配置失败：{}：{err}", location_path.display()))?;
    let trimmed = config.data_dir.trim();
    if trimmed.is_empty() {
        return Ok(default_dir);
    }
    Ok(PathBuf::from(trimmed))
}

fn write_data_location_at(default_dir: &Path, data_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(&default_dir).map_err(|err| format!("创建默认数据目录失败：{err}"))?;
    let location_path = default_dir.join(DATA_LOCATION_FILE_NAME);
    let default_dir = normalize_existing_dir(&default_dir)?;
    let data_dir = normalize_existing_dir(data_dir)?;

    if data_dir == default_dir {
        if location_path.exists() {
            fs::remove_file(&location_path).map_err(|err| {
                format!("移除数据目录配置失败：{}：{err}", location_path.display())
            })?;
        }
        return Ok(());
    }

    let text = serde_json::to_string_pretty(&DataLocationConfig {
        data_dir: path_to_string(&data_dir),
    })
    .map_err(|err| format!("序列化数据目录配置失败：{err}"))?;
    fs::write(&location_path, text)
        .map_err(|err| format!("写入数据目录配置失败：{}：{err}", location_path.display()))
}

fn move_app_data_files(
    current_data_dir: &Path,
    target_dir: PathBuf,
    default_data_dir: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    let target_dir = normalize_target_data_dir(target_dir)?;
    let current_dir = normalize_existing_dir(current_data_dir)?;

    if target_dir == current_dir {
        return Err("新数据目录和当前数据目录相同".to_string());
    }
    if target_dir.starts_with(&current_dir) {
        return Err("新数据目录不能放在当前数据目录里面".to_string());
    }

    fs::create_dir_all(&target_dir).map_err(|err| format!("创建新数据目录失败：{err}"))?;
    copy_dir_contents(&current_dir, &target_dir)?;
    write_data_location_at(default_data_dir, &target_dir)?;
    Ok((current_dir, target_dir))
}

fn normalize_target_data_dir(path: PathBuf) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("没有选择新的数据目录".to_string());
    }
    fs::create_dir_all(&path)
        .map_err(|err| format!("创建新数据目录失败：{}：{err}", path.display()))?;
    normalize_existing_dir(&path)
}

fn normalize_existing_dir(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|err| format!("读取目录路径失败：{}：{err}", path.display()))
}

fn normalize_existing_or_create_dir(path: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(path).map_err(|err| format!("创建目录失败：{}：{err}", path.display()))?;
    normalize_existing_dir(path)
}

fn copy_dir_contents(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|err| format!("创建目标目录失败：{}：{err}", target.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|err| format!("读取当前数据目录失败：{}：{err}", source.display()))?
    {
        let entry = entry.map_err(|err| format!("读取当前数据目录失败：{err}"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|err| format!("读取文件类型失败：{}：{err}", source_path.display()))?;
        if file_type.is_dir() {
            copy_dir_contents(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).map_err(|err| {
                format!(
                    "复制数据文件失败：{} -> {}：{err}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn ensure_app_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text_content TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            archived INTEGER DEFAULT 0,
            archived_at TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS migration_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            migrated_at TEXT NOT NULL,
            legacy_db_path TEXT,
            legacy_images_dir TEXT,
            legacy_message_count INTEGER NOT NULL,
            legacy_image_count INTEGER NOT NULL
        );
        ",
    )
    .map_err(|err| format!("初始化应用数据库结构失败：{err}"))?;
    ensure_legacy_schema(conn)
}

fn has_migration_state(conn: &Connection) -> Result<bool, String> {
    let count = query_count(conn, "SELECT COUNT(*) FROM migration_state WHERE id = 1")?;
    Ok(count > 0)
}

fn migrate_legacy_once(conn: &mut Connection, paths: &AppPaths) -> Result<(), String> {
    merge_legacy_data(conn, paths).map(|_| ())
}

fn merge_legacy_data(
    conn: &mut Connection,
    paths: &AppPaths,
) -> Result<AppMigrationResult, String> {
    let legacy_dir = legacy_data_dir()?;
    merge_legacy_data_from_dir(conn, paths, legacy_dir)
}

fn merge_legacy_data_from_dir(
    conn: &mut Connection,
    paths: &AppPaths,
    legacy_dir: PathBuf,
) -> Result<AppMigrationResult, String> {
    let legacy_db_path = legacy_dir.join("clipstash.db");
    let legacy_images_dir = legacy_dir.join("images");

    if !legacy_db_path.is_file() {
        mark_migrated(conn, None, None, 0, 0)?;
        return Ok(AppMigrationResult {
            inserted_messages: 0,
            skipped_messages: 0,
            copied_images: 0,
            skipped_images: 0,
            legacy_message_count: 0,
            legacy_image_count: 0,
            stats: read_app_stats_from_paths(paths)?,
        });
    }

    let legacy_stats = read_legacy_stats_from_dir(legacy_dir.clone())?;
    let mut messages = Vec::new();
    collect_legacy_messages(&legacy_dir, MessageView::Normal, &mut messages)?;
    collect_legacy_messages(&legacy_dir, MessageView::Archived, &mut messages)?;
    messages.sort_by_key(|message| message.id);

    let tx = conn
        .transaction()
        .map_err(|err| format!("开启数据迁移事务失败：{err}"))?;
    let mut image_count = 0_i64;
    let mut inserted_messages = 0_i64;
    let mut skipped_messages = 0_i64;
    let mut copied_images = 0_i64;
    let mut skipped_images = 0_i64;

    for message in messages {
        if message_already_migrated(&tx, &message)? {
            skipped_messages += 1;
            image_count += message.images.len() as i64;
            continue;
        }

        let message_id = if message_id_is_free(&tx, message.id)? {
            tx.execute(
                "INSERT INTO messages (id, text_content, created_at, archived, archived_at)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    message.id,
                    message.text_content,
                    message.created_at,
                    if message.archived { 1 } else { 0 },
                    message.archived_at
                ],
            )
            .map_err(|err| format!("迁移消息失败：{err}"))?;
            message.id
        } else {
            tx.execute(
                "INSERT INTO messages (text_content, created_at, archived, archived_at)
                 VALUES (?, ?, ?, ?)",
                params![
                    message.text_content,
                    message.created_at,
                    if message.archived { 1 } else { 0 },
                    message.archived_at
                ],
            )
            .map_err(|err| format!("迁移消息失败：{err}"))?;
            tx.last_insert_rowid()
        };
        inserted_messages += 1;

        for image in message.images {
            let image_copy = copy_legacy_image_if_present(
                &legacy_images_dir,
                &paths.images_dir,
                message_id,
                image.id,
                &image.filename,
            )?;
            tx.execute(
                "INSERT INTO message_images (message_id, image_filename)
                 VALUES (?, ?)",
                params![message_id, image_copy.filename],
            )
            .map_err(|err| format!("迁移图片关联失败：{err}"))?;
            if image_copy.copied {
                copied_images += 1;
            } else {
                skipped_images += 1;
            }
            image_count += 1;
        }
    }

    tx.execute(
        "INSERT INTO migration_state (
            id, migrated_at, legacy_db_path, legacy_images_dir, legacy_message_count, legacy_image_count
         ) VALUES (1, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            migrated_at = excluded.migrated_at,
            legacy_db_path = excluded.legacy_db_path,
            legacy_images_dir = excluded.legacy_images_dir,
            legacy_message_count = excluded.legacy_message_count,
            legacy_image_count = excluded.legacy_image_count",
        params![
            Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            path_to_string(&legacy_db_path),
            path_to_string(&legacy_images_dir),
            legacy_stats.total_count,
            image_count,
        ],
    )
    .map_err(|err| format!("写入迁移状态失败：{err}"))?;
    tx.commit()
        .map_err(|err| format!("提交数据迁移失败：{err}"))?;

    Ok(AppMigrationResult {
        inserted_messages,
        skipped_messages,
        copied_images,
        skipped_images,
        legacy_message_count: legacy_stats.total_count,
        legacy_image_count: image_count,
        stats: read_app_stats_from_paths(paths)?,
    })
}

fn message_already_migrated(conn: &Connection, message: &LegacyMessage) -> Result<bool, String> {
    if let Some(existing) = read_message_signature_by_id(conn, message.id)? {
        if existing == message_signature(message) {
            return Ok(true);
        }
    }

    let archived_value = if message.archived { 1 } else { 0 };
    let mut stmt = conn
        .prepare(
            "SELECT id FROM messages
             WHERE text_content IS ? AND created_at = ? AND archived = ? AND archived_at IS ?",
        )
        .map_err(|err| format!("准备迁移去重查询失败：{err}"))?;
    let rows = stmt
        .query_map(
            params![
                message.text_content,
                message.created_at,
                archived_value,
                message.archived_at
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|err| format!("查询迁移重复消息失败：{err}"))?;

    let mut candidate_ids = Vec::new();
    for row in rows {
        candidate_ids.push(row.map_err(|err| format!("读取迁移重复消息失败：{err}"))?);
    }
    drop(stmt);

    let target_signature = message_signature(message);
    for candidate_id in candidate_ids {
        if read_message_signature_by_id(conn, candidate_id)?.as_ref() == Some(&target_signature) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn message_id_is_free(conn: &Connection, message_id: i64) -> Result<bool, String> {
    let existing = conn
        .query_row(
            "SELECT id FROM messages WHERE id = ?",
            [message_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| format!("检查迁移消息 id 失败：{err}"))?;
    Ok(existing.is_none())
}

fn read_message_signature_by_id(
    conn: &Connection,
    message_id: i64,
) -> Result<Option<MessageSignature>, String> {
    let row = conn
        .query_row(
            "SELECT text_content, created_at, archived, archived_at FROM messages WHERE id = ?",
            [message_id],
            |row| {
                let archived: i64 = row.get(2)?;
                Ok(MessageSignature {
                    text_content: row.get(0)?,
                    created_at: row.get(1)?,
                    archived: archived == 1,
                    archived_at: row.get(3)?,
                    images: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取迁移消息签名失败：{err}"))?;

    let Some(mut signature) = row else {
        return Ok(None);
    };
    signature.images = read_image_filenames(conn, message_id)?;
    Ok(Some(signature))
}

#[derive(PartialEq, Eq)]
struct MessageSignature {
    text_content: Option<String>,
    created_at: String,
    archived: bool,
    archived_at: Option<String>,
    images: Vec<String>,
}

fn message_signature(message: &LegacyMessage) -> MessageSignature {
    MessageSignature {
        text_content: message.text_content.clone(),
        created_at: message.created_at.clone(),
        archived: message.archived,
        archived_at: message.archived_at.clone(),
        images: message
            .images
            .iter()
            .map(|image| image.filename.clone())
            .collect(),
    }
}

fn read_image_filenames(conn: &Connection, message_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT image_filename FROM message_images WHERE message_id = ? ORDER BY id")
        .map_err(|err| format!("准备迁移图片签名查询失败：{err}"))?;
    let rows = stmt
        .query_map([message_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("查询迁移图片签名失败：{err}"))?;

    let mut filenames = Vec::new();
    for row in rows {
        filenames.push(row.map_err(|err| format!("读取迁移图片签名失败：{err}"))?);
    }
    Ok(filenames)
}

struct ImageCopyResult {
    filename: String,
    copied: bool,
}

fn copy_legacy_image_if_present(
    legacy_images_dir: &Path,
    app_images_dir: &Path,
    message_id: i64,
    image_id: i64,
    filename: &str,
) -> Result<ImageCopyResult, String> {
    let source = legacy_images_dir.join(filename);
    if !source.is_file() {
        return Ok(ImageCopyResult {
            filename: filename.to_string(),
            copied: false,
        });
    }

    let mut target_filename = filename.to_string();
    let mut target = app_images_dir.join(&target_filename);
    if target.is_file() && same_file_bytes(&source, &target)? {
        return Ok(ImageCopyResult {
            filename: target_filename,
            copied: false,
        });
    }

    if target.exists() {
        target_filename =
            unique_migrated_image_filename(app_images_dir, message_id, image_id, filename);
        target = app_images_dir.join(&target_filename);
    }

    fs::copy(&source, &target).map_err(|err| {
        format!(
            "复制迁移图片失败：{} -> {}：{err}",
            source.display(),
            target.display()
        )
    })?;
    Ok(ImageCopyResult {
        filename: target_filename,
        copied: true,
    })
}

fn same_file_bytes(left: &Path, right: &Path) -> Result<bool, String> {
    let left_meta = left
        .metadata()
        .map_err(|err| format!("读取迁移图片信息失败：{}：{err}", left.display()))?;
    let right_meta = right
        .metadata()
        .map_err(|err| format!("读取迁移图片信息失败：{}：{err}", right.display()))?;
    if left_meta.len() != right_meta.len() {
        return Ok(false);
    }
    let left_bytes =
        fs::read(left).map_err(|err| format!("读取迁移图片失败：{}：{err}", left.display()))?;
    let right_bytes =
        fs::read(right).map_err(|err| format!("读取迁移图片失败：{}：{err}", right.display()))?;
    Ok(left_bytes == right_bytes)
}

fn unique_migrated_image_filename(
    app_images_dir: &Path,
    message_id: i64,
    image_id: i64,
    filename: &str,
) -> String {
    let safe_name = Path::new(filename)
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "image.png".to_string());
    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = format!("migrated-{message_id}-{image_id}{suffix}-{safe_name}");
        if !app_images_dir.join(&candidate).exists() {
            return candidate;
        }
    }

    unreachable!("migrated image filename suffix search is unbounded");
}

fn mark_migrated(
    conn: &Connection,
    legacy_db_path: Option<String>,
    legacy_images_dir: Option<String>,
    message_count: i64,
    image_count: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO migration_state (
            id, migrated_at, legacy_db_path, legacy_images_dir, legacy_message_count, legacy_image_count
         ) VALUES (1, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            migrated_at = excluded.migrated_at,
            legacy_db_path = excluded.legacy_db_path,
            legacy_images_dir = excluded.legacy_images_dir,
            legacy_message_count = excluded.legacy_message_count,
            legacy_image_count = excluded.legacy_image_count",
        params![
            Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            legacy_db_path,
            legacy_images_dir,
            message_count,
            image_count,
        ],
    )
    .map_err(|err| format!("写入迁移状态失败：{err}"))?;
    Ok(())
}

fn collect_legacy_messages(
    legacy_dir: &Path,
    view: MessageView,
    messages: &mut Vec<LegacyMessage>,
) -> Result<(), String> {
    let mut offset = 0;
    loop {
        let page = list_legacy_messages_from_dir(
            legacy_dir.to_path_buf(),
            view,
            SortOrder::Oldest,
            Some(offset),
            Some(100),
        )?;
        offset += page.messages.len() as i64;
        messages.extend(page.messages);
        if !page.has_more {
            break;
        }
    }
    Ok(())
}

fn read_app_stats_from_paths(paths: &AppPaths) -> Result<AppStats, String> {
    let conn =
        Connection::open(&paths.db_path).map_err(|err| format!("打开应用数据库失败：{err}"))?;
    let normal_count = query_count(
        &conn,
        "SELECT COUNT(*) FROM messages WHERE archived = 0 OR archived IS NULL",
    )?;
    let archived_count = query_count(&conn, "SELECT COUNT(*) FROM messages WHERE archived = 1")?;
    let total_count = query_count(&conn, "SELECT COUNT(*) FROM messages")?;

    Ok(AppStats {
        data_dir: path_to_string(&paths.data_dir),
        db_path: path_to_string(&paths.db_path),
        images_dir: path_to_string(&paths.images_dir),
        db_exists: paths.db_path.is_file(),
        images_dir_exists: paths.images_dir.is_dir(),
        normal_count,
        archived_count,
        total_count,
    })
}

fn write_result(operation: &str, message: LegacyMessage) -> LegacyCreateTextMessageResult {
    LegacyCreateTextMessageResult {
        backup: empty_backup(Path::new("")),
        audit: audit(operation, message.id),
        message,
    }
}

fn empty_backup(source_path: &Path) -> LegacyDbBackup {
    LegacyDbBackup {
        source_path: path_to_string(source_path),
        backup_path: String::new(),
        bytes_copied: 0,
    }
}

fn audit(operation: &str, message_id: i64) -> LegacyWriteAudit {
    LegacyWriteAudit {
        operation: operation.to_string(),
        message_id,
        db_backup_path: String::new(),
        image_backup_dir: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_test_support::tiny_png_bytes;
    use std::{fs, path::Path, process};

    fn reset_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
        fs::create_dir_all(path).unwrap();
    }

    fn create_paths(base: &Path) -> AppPaths {
        let data_dir = base.join("ClipStash Next");
        AppPaths {
            db_path: data_dir.join(APP_DB_NAME),
            images_dir: data_dir.join("images"),
            data_dir,
        }
    }

    fn seed_legacy_data_dir(base: &Path) -> PathBuf {
        let legacy_dir = base.join("ClipStash");
        fs::create_dir_all(legacy_dir.join("images")).unwrap();
        fs::write(
            legacy_dir.join("images").join("legacy-one.png"),
            tiny_png_bytes(),
        )
        .unwrap();

        let conn = Connection::open(legacy_dir.join("clipstash.db")).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            CREATE TABLE message_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                image_filename TEXT NOT NULL
            );
            INSERT INTO messages (id, text_content, created_at, archived, archived_at) VALUES
                (1, 'legacy text', '2024-02-01 10:00:00', 0, NULL),
                (2, 'legacy archived', '2024-02-02 10:00:00', 1, '2024-02-03 11:00:00');
            INSERT INTO message_images (id, message_id, image_filename) VALUES
                (10, 1, 'legacy-one.png'),
                (11, 2, 'missing-legacy.png');
            ",
        )
        .unwrap();
        legacy_dir
    }

    #[test]
    fn migrates_legacy_data_once_and_skips_duplicates_without_touching_legacy_files() {
        let base = env::temp_dir().join(format!(
            "clipstash-next-app-migration-test-{}",
            process::id()
        ));
        reset_dir(&base);
        let legacy_dir = seed_legacy_data_dir(&base);
        let legacy_db = legacy_dir.join("clipstash.db");
        let legacy_image = legacy_dir.join("images").join("legacy-one.png");
        let legacy_db_before = fs::read(&legacy_db).unwrap();
        let legacy_image_before = fs::read(&legacy_image).unwrap();

        let paths = create_paths(&base);
        fs::create_dir_all(&paths.images_dir).unwrap();
        let mut conn = Connection::open(&paths.db_path).unwrap();
        ensure_app_schema(&conn).unwrap();

        let first = merge_legacy_data_from_dir(&mut conn, &paths, legacy_dir.clone()).unwrap();

        assert_eq!(first.inserted_messages, 2);
        assert_eq!(first.skipped_messages, 0);
        assert_eq!(first.copied_images, 1);
        assert_eq!(first.skipped_images, 1);
        assert_eq!(first.legacy_message_count, 2);
        assert_eq!(first.legacy_image_count, 2);
        assert_eq!(first.stats.normal_count, 1);
        assert_eq!(first.stats.archived_count, 1);
        assert!(paths.images_dir.join("legacy-one.png").is_file());

        let second = merge_legacy_data_from_dir(&mut conn, &paths, legacy_dir.clone()).unwrap();

        assert_eq!(second.inserted_messages, 0);
        assert_eq!(second.skipped_messages, 2);
        assert_eq!(second.copied_images, 0);
        assert_eq!(second.skipped_images, 0);
        assert_eq!(second.stats.total_count, 2);
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").unwrap(),
            2
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM migration_state WHERE id = 1").unwrap(),
            1
        );
        assert_eq!(fs::read(&legacy_db).unwrap(), legacy_db_before);
        assert_eq!(fs::read(&legacy_image).unwrap(), legacy_image_before);

        drop(conn);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn moves_app_data_to_selected_dir_without_deleting_previous_files() {
        let base = env::temp_dir().join(format!(
            "clipstash-next-app-data-move-test-{}",
            process::id()
        ));
        reset_dir(&base);

        let default_dir = base.join(APP_DATA_DIR_NAME);
        let current_dir = base.join("Current ClipStash Data");
        fs::create_dir_all(current_dir.join("images")).unwrap();
        fs::write(current_dir.join(APP_DB_NAME), b"db").unwrap();
        fs::write(current_dir.join("images").join("one.png"), tiny_png_bytes()).unwrap();
        fs::write(current_dir.join("settings.json"), b"{\"sort\":\"oldest\"}").unwrap();
        let current_db_bytes = fs::read(current_dir.join(APP_DB_NAME)).unwrap();

        let target_dir = base.join("Moved ClipStash Data");
        let (previous_data_dir, moved_data_dir) =
            move_app_data_files(&current_dir, target_dir.clone(), &default_dir).unwrap();

        assert_eq!(
            path_to_string(&previous_data_dir),
            path_to_string(&current_dir.canonicalize().unwrap())
        );
        assert_eq!(
            path_to_string(&moved_data_dir),
            path_to_string(&target_dir.canonicalize().unwrap())
        );
        assert!(current_dir.join(APP_DB_NAME).is_file());
        assert!(current_dir.join("images").join("one.png").is_file());
        assert_eq!(
            fs::read(target_dir.join(APP_DB_NAME)).unwrap(),
            current_db_bytes
        );
        assert!(target_dir.join("images").join("one.png").is_file());

        let location_text =
            fs::read_to_string(base.join(APP_DATA_DIR_NAME).join(DATA_LOCATION_FILE_NAME)).unwrap();
        assert!(location_text.contains("Moved ClipStash Data"));

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn repairs_moved_app_data_by_copying_missing_files_from_default_dir() {
        let base = env::temp_dir().join(format!(
            "clipstash-next-app-data-repair-test-{}",
            process::id()
        ));
        reset_dir(&base);

        let default_dir = base.join(APP_DATA_DIR_NAME);
        let current_dir = base.join("Moved ClipStash Data");
        fs::create_dir_all(default_dir.join("images")).unwrap();
        fs::create_dir_all(current_dir.join("images")).unwrap();
        fs::write(
            default_dir.join("images").join("missing.png"),
            b"from-default",
        )
        .unwrap();
        fs::write(default_dir.join("images").join("kept.png"), b"default-kept").unwrap();
        fs::write(current_dir.join("images").join("kept.png"), b"current-kept").unwrap();

        let current_paths = AppPaths {
            data_dir: current_dir.clone(),
            db_path: current_dir.join(APP_DB_NAME),
            images_dir: current_dir.join("images"),
        };
        let default_conn = Connection::open(default_dir.join(APP_DB_NAME)).unwrap();
        ensure_app_schema(&default_conn).unwrap();
        drop(default_conn);
        let default_db_bytes = fs::read(default_dir.join(APP_DB_NAME)).unwrap();

        let result = repair_app_data_dir_from_default(&current_paths, &default_dir).unwrap();

        assert!(result.copied_db);
        assert_eq!(result.copied_images, 1);
        assert_eq!(result.skipped_images, 1);
        assert_eq!(
            fs::read(current_dir.join("images").join("missing.png")).unwrap(),
            b"from-default"
        );
        assert_eq!(
            fs::read(current_dir.join("images").join("kept.png")).unwrap(),
            b"current-kept"
        );
        assert_eq!(
            fs::read(current_dir.join(APP_DB_NAME)).unwrap(),
            default_db_bytes
        );

        fs::remove_dir_all(base).unwrap();
    }
}
