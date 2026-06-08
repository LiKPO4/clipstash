use arboard::{Clipboard, ImageData};
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_MESSAGE_LIMIT: i64 = 30;
const MAX_MESSAGE_LIMIT: i64 = 100;

#[derive(Serialize)]
pub struct LegacyStats {
    pub data_dir: String,
    pub db_path: String,
    pub images_dir: String,
    pub db_exists: bool,
    pub images_dir_exists: bool,
    pub normal_count: i64,
    pub archived_count: i64,
    pub total_count: i64,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageView {
    Normal,
    Archived,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Newest,
    Oldest,
}

#[derive(Clone, Debug, Serialize)]
pub struct LegacyMessageImage {
    pub id: i64,
    pub filename: String,
    pub path: String,
    pub exists: bool,
}

#[derive(Serialize)]
pub struct LegacyMessage {
    pub id: i64,
    pub text_content: Option<String>,
    pub created_at: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub images: Vec<LegacyMessageImage>,
}

#[derive(Serialize)]
pub struct LegacyMessagePage {
    pub view: String,
    pub sort: String,
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub has_more: bool,
    pub messages: Vec<LegacyMessage>,
}

#[derive(Serialize)]
pub struct LegacyDbBackup {
    pub source_path: String,
    pub backup_path: String,
    pub bytes_copied: u64,
}

#[derive(Serialize)]
pub struct LegacyCreateTextMessageResult {
    pub backup: LegacyDbBackup,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyUpdateMessageResult {
    pub backup: LegacyDbBackup,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyImageFilesBackup {
    pub backup_dir: String,
    pub filenames: Vec<String>,
}

#[derive(Serialize)]
pub struct LegacyReplaceImagesResult {
    pub backup: LegacyDbBackup,
    pub image_backup: Option<LegacyImageFilesBackup>,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyDeleteMessageResult {
    pub backup: LegacyDbBackup,
    pub image_backup: Option<LegacyImageFilesBackup>,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyArchiveMessageResult {
    pub backup: LegacyDbBackup,
    pub message: LegacyMessage,
}

#[derive(Debug, Serialize)]
pub struct LegacyCopyImageResult {
    pub filename: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize)]
pub struct LegacyImportStageResult {
    pub message_id: i64,
    pub staged_kind: String,
    pub text_length: usize,
    pub image_count: usize,
    pub first_image_filename: Option<String>,
    pub copied_image: Option<LegacyCopyImageResult>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueueItem {
    pub kind: String,
    pub text: Option<String>,
    pub text_length: usize,
    pub image: Option<LegacyMessageImage>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueuePreview {
    pub message_id: i64,
    pub item_count: usize,
    pub text_length: usize,
    pub image_count: usize,
    pub skipped_missing_image_count: usize,
    pub items: Vec<LegacyImportQueueItem>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueueCopyResult {
    pub message_id: i64,
    pub item_index: usize,
    pub staged_kind: String,
    pub text_length: usize,
    pub image_filename: Option<String>,
    pub copied_image: Option<LegacyCopyImageResult>,
}

pub fn read_legacy_stats() -> Result<LegacyStats, String> {
    let data_dir = legacy_data_dir()?;
    read_legacy_stats_from_dir(data_dir)
}

pub fn create_legacy_db_backup() -> Result<LegacyDbBackup, String> {
    let data_dir = legacy_data_dir()?;
    create_legacy_db_backup_for_path(&data_dir.join("clipstash.db"))
}

pub fn create_legacy_text_message(
    text_content: String,
) -> Result<LegacyCreateTextMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    create_text_message_with_backup_for_path(&data_dir.join("clipstash.db"), text_content)
}

pub fn create_legacy_image_message(
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    create_image_message_with_backup_for_path(&data_dir.join("clipstash.db"), images_data)
}

pub fn create_legacy_mixed_message(
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    create_mixed_message_with_backup_for_path(
        &data_dir.join("clipstash.db"),
        text_content,
        images_data,
    )
}

pub fn update_legacy_message_text(
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyUpdateMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    update_text_message_with_backup_for_path(
        &data_dir.join("clipstash.db"),
        message_id,
        text_content,
    )
}

pub fn replace_legacy_message_images(
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyReplaceImagesResult, String> {
    let data_dir = legacy_data_dir()?;
    replace_message_images_with_backup_for_path(
        &data_dir.join("clipstash.db"),
        message_id,
        images_data,
    )
}

pub fn delete_legacy_message(message_id: i64) -> Result<LegacyDeleteMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    delete_message_with_backup_for_path(&data_dir.join("clipstash.db"), message_id)
}

pub fn set_legacy_message_archived(
    message_id: i64,
    archived: bool,
) -> Result<LegacyArchiveMessageResult, String> {
    let data_dir = legacy_data_dir()?;
    set_message_archived_with_backup_for_path(&data_dir.join("clipstash.db"), message_id, archived)
}

pub fn copy_legacy_image_to_clipboard(filename: String) -> Result<LegacyCopyImageResult, String> {
    let data_dir = legacy_data_dir()?;
    copy_legacy_image_to_clipboard_from_dir(data_dir, filename)
}

pub fn stage_legacy_message_import_to_clipboard(
    message_id: i64,
) -> Result<LegacyImportStageResult, String> {
    let data_dir = legacy_data_dir()?;
    stage_legacy_message_import_to_clipboard_from_dir(data_dir, message_id)
}

pub fn preview_legacy_message_import_queue(
    message_id: i64,
) -> Result<LegacyImportQueuePreview, String> {
    let data_dir = legacy_data_dir()?;
    preview_legacy_message_import_queue_from_dir(data_dir, message_id)
}

pub fn copy_legacy_message_import_queue_item_to_clipboard(
    message_id: i64,
    item_index: usize,
) -> Result<LegacyImportQueueCopyResult, String> {
    let data_dir = legacy_data_dir()?;
    copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir, message_id, item_index)
}

pub fn list_legacy_messages(
    view: MessageView,
    sort: SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LegacyMessagePage, String> {
    let data_dir = legacy_data_dir()?;
    list_legacy_messages_from_dir(data_dir, view, sort, offset, limit)
}

fn create_legacy_db_backup_for_path(db_path: &Path) -> Result<LegacyDbBackup, String> {
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

fn read_legacy_stats_from_dir(data_dir: PathBuf) -> Result<LegacyStats, String> {
    let db_path = data_dir.join("clipstash.db");
    let images_dir = data_dir.join("images");
    let db_exists = db_path.is_file();
    let images_dir_exists = images_dir.is_dir();

    if !db_exists {
        return Err(format!("未找到旧数据库：{}", db_path.display()));
    }

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("只读打开旧数据库失败：{err}"))?;

    ensure_legacy_schema(&conn)?;

    let normal_count = query_count(
        &conn,
        "SELECT COUNT(*) FROM messages WHERE archived = 0 OR archived IS NULL",
    )?;
    let archived_count = query_count(&conn, "SELECT COUNT(*) FROM messages WHERE archived = 1")?;
    let total_count = query_count(&conn, "SELECT COUNT(*) FROM messages")?;

    Ok(LegacyStats {
        data_dir: path_to_string(data_dir),
        db_path: path_to_string(db_path),
        images_dir: path_to_string(images_dir),
        db_exists,
        images_dir_exists,
        normal_count,
        archived_count,
        total_count,
    })
}

fn list_legacy_messages_from_dir(
    data_dir: PathBuf,
    view: MessageView,
    sort: SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LegacyMessagePage, String> {
    let db_path = data_dir.join("clipstash.db");
    let images_dir = data_dir.join("images");

    if !db_path.is_file() {
        return Err(format!("未找到旧数据库：{}", db_path.display()));
    }

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("只读打开旧数据库失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let offset = offset.unwrap_or(0).max(0);
    let limit = limit
        .unwrap_or(DEFAULT_MESSAGE_LIMIT)
        .clamp(1, MAX_MESSAGE_LIMIT);
    let total_count = query_count(&conn, view_count_sql(view))?;
    let order = match sort {
        SortOrder::Newest => "DESC",
        SortOrder::Oldest => "ASC",
    };
    let sort_column = match view {
        MessageView::Normal => "created_at",
        MessageView::Archived => "COALESCE(archived_at, created_at)",
    };
    let sql = format!(
        "SELECT id, text_content, created_at, archived, archived_at \
         FROM messages \
         WHERE {} \
         ORDER BY {sort_column} {order}, id {order} \
         LIMIT ? OFFSET ?",
        view_where_sql(view)
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("准备旧消息查询失败：{err}"))?;
    let rows = stmt
        .query_map(params![limit, offset], |row| {
            let archived: i64 = row.get(3)?;
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                archived == 1,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|err| format!("查询旧消息失败：{err}"))?;

    let mut messages = Vec::new();
    for row in rows {
        let (id, text_content, created_at, archived, archived_at) =
            row.map_err(|err| format!("读取旧消息行失败：{err}"))?;
        let images = list_images_for_message(&conn, &images_dir, id)?;
        messages.push(LegacyMessage {
            id,
            text_content,
            created_at,
            archived,
            archived_at,
            images,
        });
    }

    let has_more = offset + (messages.len() as i64) < total_count;

    Ok(LegacyMessagePage {
        view: view_key(view).to_string(),
        sort: sort_key(sort).to_string(),
        offset,
        limit,
        total_count,
        has_more,
        messages,
    })
}

fn create_text_message_with_backup_for_path(
    db_path: &Path,
    text_content: String,
) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized_text = normalize_text_message(text_content)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_text_message_for_path(db_path, Some(normalized_text))
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    Ok(LegacyCreateTextMessageResult { backup, message })
}

fn create_image_message_with_backup_for_path(
    db_path: &Path,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    validate_images_data(&images_data)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_image_message_for_path(db_path, images_data)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    Ok(LegacyCreateTextMessageResult { backup, message })
}

fn create_mixed_message_with_backup_for_path(
    db_path: &Path,
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized_text = normalize_text_message(text_content)?;
    validate_images_data(&images_data)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_mixed_message_for_path(db_path, Some(normalized_text), images_data)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    Ok(LegacyCreateTextMessageResult { backup, message })
}

fn update_text_message_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyUpdateMessageResult, String> {
    let normalized_text = normalize_optional_text_message(text_content);
    ensure_message_exists_for_path(db_path, message_id)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = update_text_message_for_path(db_path, message_id, normalized_text)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    Ok(LegacyUpdateMessageResult { backup, message })
}

fn replace_message_images_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyReplaceImagesResult, String> {
    validate_replace_images_request(db_path, message_id, &images_data)?;
    let current_message = read_message_for_update_precheck(db_path, message_id)?;
    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "替换消息图片失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let image_backup = backup_message_image_files(data_dir, &current_message.images)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;
    let message = replace_message_images_for_path(db_path, message_id, images_data)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;

    Ok(LegacyReplaceImagesResult {
        backup,
        image_backup,
        message,
    })
}

fn delete_message_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
) -> Result<LegacyDeleteMessageResult, String> {
    let current_message = read_message_for_update_precheck(db_path, message_id)?;
    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("删除消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let image_backup = backup_message_image_files(data_dir, &current_message.images)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;
    let message = delete_message_for_path(db_path, message_id)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;

    Ok(LegacyDeleteMessageResult {
        backup,
        image_backup,
        message,
    })
}

fn set_message_archived_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    archived: bool,
) -> Result<LegacyArchiveMessageResult, String> {
    ensure_message_exists_for_path(db_path, message_id)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = set_message_archived_for_path(db_path, message_id, archived)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    Ok(LegacyArchiveMessageResult { backup, message })
}

pub fn create_text_message_for_path(
    db_path: &Path,
    text_content: Option<String>,
) -> Result<LegacyMessage, String> {
    if !db_path.is_file() {
        return Err(format!("新增消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("新增消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备写入失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    conn.execute(
        "INSERT INTO messages (text_content, archived) VALUES (?, 0)",
        params![text_content],
    )
    .map_err(|err| format!("新增纯文字消息失败：{err}"))?;

    let message_id = conn.last_insert_rowid();
    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub fn replace_message_images_for_path(
    db_path: &Path,
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyMessage, String> {
    validate_replace_images_request(db_path, message_id, &images_data)?;

    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "替换消息图片失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let images_dir = data_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|err| format!("创建旧图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备替换图片失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let mut saved_paths = Vec::new();
    let replace_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启图片替换事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![message_id],
        )
        .map_err(|err| format!("删除旧图片关联失败：{err}"))?;

        for (index, image_data) in images_data.iter().enumerate() {
            let filename = next_image_filename(&images_dir, index);
            let path = images_dir.join(&filename);
            saved_paths.push(path.clone());
            save_image_file(&path, image_data)?;
            tx.execute(
                "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                params![message_id, filename],
            )
            .map_err(|err| format!("新增图片关联失败：{err}"))?;
        }

        tx.commit()
            .map_err(|err| format!("提交图片替换失败：{err}"))
    })();

    if let Err(err) = replace_result {
        for path in saved_paths {
            let _ = fs::remove_file(path);
        }
        return Err(err);
    }

    remove_old_message_image_files(&old_message.images);
    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub fn delete_message_for_path(db_path: &Path, message_id: i64) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("删除消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("删除消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("删除消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备删除失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let delete_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启删除消息事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![message_id],
        )
        .map_err(|err| format!("删除图片关联失败：{err}"))?;
        let deleted = tx
            .execute("DELETE FROM messages WHERE id = ?", params![message_id])
            .map_err(|err| format!("删除消息失败：{err}"))?;
        if deleted == 0 {
            return Err(format!("删除消息失败，消息不存在：{message_id}"));
        }
        tx.commit().map_err(|err| format!("提交删除失败：{err}"))
    })();

    delete_result?;
    remove_old_message_image_files(&old_message.images);
    Ok(old_message)
}

pub fn set_message_archived_for_path(
    db_path: &Path,
    message_id: i64,
    archived: bool,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("更新归档状态失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!(
            "更新归档状态失败，数据库不存在：{}",
            db_path.display()
        ));
    }

    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "更新归档状态失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let images_dir = data_dir.join("images");
    let conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备更新归档失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let archived_value = if archived { 1 } else { 0 };
    let archived_at = if archived {
        Some(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    } else {
        None
    };
    let updated = conn
        .execute(
            "UPDATE messages SET archived = ?, archived_at = ? WHERE id = ?",
            params![archived_value, archived_at, message_id],
        )
        .map_err(|err| format!("更新归档状态失败：{err}"))?;

    if updated == 0 {
        return Err(format!("更新归档状态失败，消息不存在：{message_id}"));
    }

    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub fn update_text_message_for_path(
    db_path: &Path,
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("更新消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("更新消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("更新消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备更新失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let updated = conn
        .execute(
            "UPDATE messages SET text_content = ? WHERE id = ?",
            params![text_content, message_id],
        )
        .map_err(|err| format!("更新消息文字失败：{err}"))?;

    if updated == 0 {
        return Err(format!("更新消息失败，消息不存在：{message_id}"));
    }

    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

#[allow(dead_code)]
pub fn create_image_message_for_path(
    db_path: &Path,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyMessage, String> {
    create_mixed_message_for_path(db_path, None, images_data)
}

#[allow(dead_code)]
pub fn create_mixed_message_for_path(
    db_path: &Path,
    text_content: Option<String>,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyMessage, String> {
    validate_images_data(&images_data)?;

    if !db_path.is_file() {
        return Err(format!(
            "新增图片消息失败，数据库不存在：{}",
            db_path.display()
        ));
    }

    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "新增图片消息失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let images_dir = data_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|err| format!("创建旧图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备写入失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let mut saved_paths = Vec::new();
    let insert_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启图片消息写入事务失败：{err}"))?;
        tx.execute(
            "INSERT INTO messages (text_content, archived) VALUES (?, 0)",
            params![text_content],
        )
        .map_err(|err| format!("新增图文消息失败：{err}"))?;

        let message_id = tx.last_insert_rowid();
        for (index, image_data) in images_data.iter().enumerate() {
            let filename = next_image_filename(&images_dir, index);
            let path = images_dir.join(&filename);
            saved_paths.push(path.clone());
            save_image_file(&path, image_data)?;
            tx.execute(
                "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                params![message_id, filename],
            )
            .map_err(|err| format!("新增图片关联失败：{err}"))?;
        }

        tx.commit()
            .map_err(|err| format!("提交图片消息写入失败：{err}"))?;
        Ok::<i64, String>(message_id)
    })();

    let message_id = match insert_result {
        Ok(message_id) => message_id,
        Err(err) => {
            for path in saved_paths {
                let _ = fs::remove_file(path);
            }
            return Err(err);
        }
    };

    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

fn normalize_text_message(text_content: String) -> Result<String, String> {
    let normalized = text_content.trim().to_string();
    if normalized.is_empty() {
        return Err("新增纯文字消息失败，文字内容不能为空".to_string());
    }

    Ok(normalized)
}

fn normalize_optional_text_message(text_content: Option<String>) -> Option<String> {
    text_content
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn validate_images_data(images_data: &[Vec<u8>]) -> Result<(), String> {
    if images_data.is_empty() {
        return Err("新增图片消息失败，至少需要一张图片".to_string());
    }
    if images_data.iter().any(|image_data| image_data.is_empty()) {
        return Err("新增图片消息失败，图片数据不能为空".to_string());
    }

    Ok(())
}

fn validate_replace_images_request(
    db_path: &Path,
    message_id: i64,
    images_data: &[Vec<u8>],
) -> Result<(), String> {
    if message_id <= 0 {
        return Err("替换消息图片失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!(
            "替换消息图片失败，数据库不存在：{}",
            db_path.display()
        ));
    }
    if images_data.iter().any(|image_data| image_data.is_empty()) {
        return Err("替换消息图片失败，图片数据不能为空".to_string());
    }

    let current_message = read_message_for_update_precheck(db_path, message_id)?;
    let has_text = current_message
        .text_content
        .as_deref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    if images_data.is_empty() && !has_text {
        return Err("替换消息图片失败，不能清空无文字消息的所有图片".to_string());
    }

    Ok(())
}

fn ensure_message_exists_for_path(db_path: &Path, message_id: i64) -> Result<(), String> {
    if message_id <= 0 {
        return Err("更新消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("更新消息失败，数据库不存在：{}", db_path.display()));
    }

    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("只读打开旧数据库检查消息失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE id = ?",
            [message_id],
            |row| row.get(0),
        )
        .map_err(|err| format!("检查消息是否存在失败：{err}"))?;

    if exists == 0 {
        return Err(format!("更新消息失败，消息不存在：{message_id}"));
    }

    Ok(())
}

fn read_message_for_update_precheck(
    db_path: &Path,
    message_id: i64,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("读取消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("读取消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("读取消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|err| format!("只读打开旧数据库检查消息失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

fn backup_message_image_files(
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

fn remove_old_message_image_files(images: &[LegacyMessageImage]) {
    for image in images {
        let _ = fs::remove_file(&image.path);
    }
}

fn save_image_file(path: &Path, image_data: &[u8]) -> Result<(), String> {
    fs::write(path, image_data).map_err(|err| format!("保存图片文件失败：{err}"))
}

fn next_image_filename(images_dir: &Path, index: usize) -> String {
    let timestamp = Local::now().format("%Y%m%d%H%M%S%3f");
    let process_id = std::process::id();

    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let filename = format!("clipstash-next-{timestamp}-{process_id}-{index}{suffix}.png");
        if !images_dir.join(&filename).exists() {
            return filename;
        }
    }

    unreachable!("image filename suffix search is unbounded");
}

fn next_backup_path(parent: &Path, timestamp: &str) -> PathBuf {
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

fn legacy_data_dir() -> Result<PathBuf, String> {
    if let Some(appdata) = env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata).join("ClipStash"));
    }

    if let Some(user_profile) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(user_profile).join("ClipStash"));
    }

    Err("无法定位 APPDATA 或 USERPROFILE，不能确定旧数据目录".to_string())
}

fn ensure_legacy_schema(conn: &Connection) -> Result<(), String> {
    let messages_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'messages'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("检查 messages 表失败：{err}"))?;

    if messages_exists == 0 {
        return Err("旧数据库缺少 messages 表".to_string());
    }

    Ok(())
}

fn copy_legacy_image_to_clipboard_from_dir(
    data_dir: PathBuf,
    filename: String,
) -> Result<LegacyCopyImageResult, String> {
    let image_path = resolve_legacy_image_path(&data_dir, &filename)?;
    let image = image::open(&image_path)
        .map_err(|err| format!("读取旧图片准备复制失败：{}：{err}", image_path.display()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let bytes = image.into_raw();

    let mut clipboard =
        Clipboard::new().map_err(|err| format!("打开系统剪贴板准备复制图片失败：{err}"))?;
    clipboard
        .set_image(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(bytes),
        })
        .map_err(|err| format!("写入图片到系统剪贴板失败：{err}"))?;

    Ok(LegacyCopyImageResult {
        filename,
        path: path_to_string(image_path),
        width,
        height,
    })
}

fn stage_legacy_message_import_to_clipboard_from_dir(
    data_dir: PathBuf,
    message_id: i64,
) -> Result<LegacyImportStageResult, String> {
    let db_path = data_dir.join("clipstash.db");
    let message = read_message_for_update_precheck(&db_path, message_id)?;
    let text = message
        .text_content
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let first_existing_image = message.images.iter().find(|image| image.exists);
    let text_length = text.map(|value| value.chars().count()).unwrap_or(0);
    let image_count = message.images.iter().filter(|image| image.exists).count();
    let first_image_filename = first_existing_image.map(|image| image.filename.clone());

    if let Some(text) = text {
        let mut clipboard =
            Clipboard::new().map_err(|err| format!("打开系统剪贴板准备导入文字失败：{err}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| format!("写入文字到系统剪贴板失败：{err}"))?;

        return Ok(LegacyImportStageResult {
            message_id,
            staged_kind: "text".to_string(),
            text_length,
            image_count,
            first_image_filename,
            copied_image: None,
        });
    }

    if let Some(image) = first_existing_image {
        let copied_image =
            copy_legacy_image_to_clipboard_from_dir(data_dir, image.filename.clone())?;
        return Ok(LegacyImportStageResult {
            message_id,
            staged_kind: "image".to_string(),
            text_length,
            image_count,
            first_image_filename,
            copied_image: Some(copied_image),
        });
    }

    Err(format!(
        "导入消息失败，消息为空或图片文件缺失：#{message_id}"
    ))
}

fn preview_legacy_message_import_queue_from_dir(
    data_dir: PathBuf,
    message_id: i64,
) -> Result<LegacyImportQueuePreview, String> {
    let db_path = data_dir.join("clipstash.db");
    let message = read_message_for_update_precheck(&db_path, message_id)?;
    import_queue_preview_from_message(message)
}

fn import_queue_preview_from_message(
    message: LegacyMessage,
) -> Result<LegacyImportQueuePreview, String> {
    let text = message
        .text_content
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let text_length = text.map(|value| value.chars().count()).unwrap_or(0);
    let existing_images: Vec<LegacyMessageImage> = message
        .images
        .iter()
        .filter(|image| image.exists)
        .cloned()
        .collect();
    let skipped_missing_image_count = message.images.len().saturating_sub(existing_images.len());

    let mut items = Vec::new();
    if let Some(text) = text {
        items.push(LegacyImportQueueItem {
            kind: "text".to_string(),
            text: Some(text.to_string()),
            text_length,
            image: None,
        });
    }
    for image in existing_images.iter().cloned() {
        items.push(LegacyImportQueueItem {
            kind: "image".to_string(),
            text: None,
            text_length: 0,
            image: Some(image),
        });
    }

    if items.is_empty() {
        return Err(format!(
            "导入消息失败，消息为空或图片文件缺失：#{}",
            message.id
        ));
    }

    Ok(LegacyImportQueuePreview {
        message_id: message.id,
        item_count: items.len(),
        text_length,
        image_count: existing_images.len(),
        skipped_missing_image_count,
        items,
    })
}

fn copy_legacy_message_import_queue_item_to_clipboard_from_dir(
    data_dir: PathBuf,
    message_id: i64,
    item_index: usize,
) -> Result<LegacyImportQueueCopyResult, String> {
    let preview = preview_legacy_message_import_queue_from_dir(data_dir.clone(), message_id)?;
    let item = preview.items.get(item_index).ok_or_else(|| {
        format!(
            "复制导入队列项失败，索引超出范围：#{message_id} index={item_index} total={}",
            preview.item_count
        )
    })?;

    if item.kind == "text" {
        let text = item
            .text
            .as_deref()
            .ok_or_else(|| format!("复制导入队列文字失败，队列项缺少文字：#{message_id}"))?;
        let mut clipboard =
            Clipboard::new().map_err(|err| format!("打开系统剪贴板准备导入文字失败：{err}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| format!("写入文字到系统剪贴板失败：{err}"))?;

        return Ok(LegacyImportQueueCopyResult {
            message_id,
            item_index,
            staged_kind: "text".to_string(),
            text_length: item.text_length,
            image_filename: None,
            copied_image: None,
        });
    }

    if item.kind == "image" {
        let image = item
            .image
            .as_ref()
            .ok_or_else(|| format!("复制导入队列图片失败，队列项缺少图片：#{message_id}"))?;
        let copied_image =
            copy_legacy_image_to_clipboard_from_dir(data_dir, image.filename.clone())?;

        return Ok(LegacyImportQueueCopyResult {
            message_id,
            item_index,
            staged_kind: "image".to_string(),
            text_length: 0,
            image_filename: Some(image.filename.clone()),
            copied_image: Some(copied_image),
        });
    }

    Err(format!("复制导入队列项失败，未知队列项类型：{}", item.kind))
}

fn resolve_legacy_image_path(data_dir: &Path, filename: &str) -> Result<PathBuf, String> {
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

#[allow(dead_code)]
fn read_legacy_message_by_id(
    conn: &Connection,
    images_dir: &PathBuf,
    message_id: i64,
) -> Result<LegacyMessage, String> {
    let (id, text_content, created_at, archived, archived_at) = conn
        .query_row(
            "SELECT id, text_content, created_at, archived, archived_at \
             FROM messages \
             WHERE id = ?",
            [message_id],
            |row| {
                let archived: i64 = row.get(3)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    archived == 1,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .map_err(|err| format!("读取新增消息失败：{err}"))?;
    let images = list_images_for_message(conn, images_dir, id)?;

    Ok(LegacyMessage {
        id,
        text_content,
        created_at,
        archived,
        archived_at,
        images,
    })
}

fn list_images_for_message(
    conn: &Connection,
    images_dir: &PathBuf,
    message_id: i64,
) -> Result<Vec<LegacyMessageImage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, image_filename \
             FROM message_images \
             WHERE message_id = ? \
             ORDER BY id",
        )
        .map_err(|err| format!("准备旧图片查询失败：{err}"))?;
    let rows = stmt
        .query_map([message_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| format!("查询旧图片失败：{err}"))?;

    let mut images = Vec::new();
    for row in rows {
        let (id, filename) = row.map_err(|err| format!("读取旧图片行失败：{err}"))?;
        let path = images_dir.join(&filename);
        images.push(LegacyMessageImage {
            id,
            filename,
            exists: path.is_file(),
            path: path_to_string(path),
        });
    }

    Ok(images)
}

fn query_count(conn: &Connection, sql: &str) -> Result<i64, String> {
    conn.query_row(sql, [], |row| row.get(0))
        .map_err(|err| format!("查询旧数据库计数失败：{err}"))
}

fn view_where_sql(view: MessageView) -> &'static str {
    match view {
        MessageView::Normal => "archived = 0 OR archived IS NULL",
        MessageView::Archived => "archived = 1",
    }
}

fn view_count_sql(view: MessageView) -> &'static str {
    match view {
        MessageView::Normal => {
            "SELECT COUNT(*) FROM messages WHERE archived = 0 OR archived IS NULL"
        }
        MessageView::Archived => "SELECT COUNT(*) FROM messages WHERE archived = 1",
    }
}

fn view_key(view: MessageView) -> &'static str {
    match view {
        MessageView::Normal => "normal",
        MessageView::Archived => "archived",
    }
}

fn sort_key(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::Newest => "newest",
        SortOrder::Oldest => "oldest",
    }
}

fn path_to_string(path: PathBuf) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process};

    #[test]
    fn reads_counts_from_legacy_messages_table() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-stats-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            INSERT INTO messages (text_content, archived) VALUES ('normal', 0);
            INSERT INTO messages (text_content, archived) VALUES ('archived', 1);
            INSERT INTO messages (text_content, archived) VALUES ('legacy-null', NULL);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let stats = read_legacy_stats_from_dir(data_dir.clone()).expect("read legacy stats");

        assert!(stats.db_exists);
        assert!(stats.images_dir_exists);
        assert_eq!(stats.normal_count, 2);
        assert_eq!(stats.archived_count, 1);
        assert_eq!(stats.total_count, 3);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn lists_messages_with_ordered_image_status() {
        let data_dir =
            env::temp_dir().join(format!("clipstash-next-legacy-list-test-{}", process::id()));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");
        fs::write(data_dir.join("images").join("existing.png"), b"png").expect("seed image");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (id, text_content, created_at, archived) VALUES
                (1, 'older', '2024-01-01 00:00:00', 0),
                (2, 'newer', '2024-02-01 00:00:00', 0),
                (3, 'archived', '2024-03-01 00:00:00', 1);
            INSERT INTO message_images (id, message_id, image_filename) VALUES
                (10, 2, 'existing.png'),
                (11, 2, 'missing.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("list normal messages");

        assert_eq!(page.total_count, 2);
        assert!(!page.has_more);
        assert_eq!(page.messages[0].id, 2);
        assert_eq!(page.messages[1].id, 1);
        assert_eq!(page.messages[0].images[0].id, 10);
        assert!(page.messages[0].images[0].exists);
        assert_eq!(page.messages[0].images[1].id, 11);
        assert!(!page.messages[0].images[1].exists);

        let archived_page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Archived,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("list archived messages");

        assert_eq!(archived_page.total_count, 1);
        assert_eq!(archived_page.messages[0].id, 3);
        assert!(archived_page.messages[0].archived);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn previews_import_queue_in_legacy_order_without_writing() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-import-queue-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");
        fs::write(data_dir.join("images").join("second.png"), tiny_png_bytes())
            .expect("seed second image");
        fs::write(data_dir.join("images").join("first.png"), tiny_png_bytes())
            .expect("seed first image");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (id, text_content, archived) VALUES
                (1, '  hello queue  ', 0),
                (2, NULL, 0);
            INSERT INTO message_images (id, message_id, image_filename) VALUES
                (21, 1, 'second.png'),
                (20, 1, 'first.png'),
                (22, 1, 'missing.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let preview = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 1)
            .expect("preview import queue");

        assert_eq!(preview.message_id, 1);
        assert_eq!(preview.item_count, 3);
        assert_eq!(preview.text_length, 11);
        assert_eq!(preview.image_count, 2);
        assert_eq!(preview.skipped_missing_image_count, 1);
        assert_eq!(preview.items[0].kind, "text");
        assert_eq!(preview.items[0].text.as_deref(), Some("hello queue"));
        assert_eq!(
            preview.items[1]
                .image
                .as_ref()
                .map(|image| image.filename.as_str()),
            Some("first.png")
        );
        assert_eq!(
            preview.items[2]
                .image
                .as_ref()
                .map(|image| image.filename.as_str()),
            Some("second.png")
        );

        let empty = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 2)
            .expect_err("empty message should fail preview");
        assert!(empty.contains("消息为空或图片文件缺失"));

        let out_of_range =
            copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 1, 3)
                .expect_err("out-of-range queue item should fail before writing clipboard");
        assert!(out_of_range.contains("索引超出范围"));

        let empty_copy =
            copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 2, 0)
                .expect_err("empty message should fail before writing clipboard");
        assert!(empty_copy.contains("消息为空或图片文件缺失"));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn creates_timestamped_legacy_db_backup_without_mutating_source() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-backup-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create backup fixture dir");

        let db_path = data_dir.join("clipstash.db");
        fs::write(&db_path, b"legacy-db-bytes").expect("write fixture db");

        let backup = create_legacy_db_backup_for_path(&db_path).expect("create db backup");
        let backup_path = PathBuf::from(&backup.backup_path);

        assert!(backup_path.is_file());
        assert!(backup_path
            .file_name()
            .expect("backup filename")
            .to_string_lossy()
            .starts_with("clipstash.db.bak-"));
        assert_eq!(backup.bytes_copied, 15);
        assert_eq!(
            fs::read(&db_path).expect("read source db"),
            b"legacy-db-bytes"
        );
        assert_eq!(
            fs::read(backup_path).expect("read backup db"),
            b"legacy-db-bytes"
        );

        fs::remove_dir_all(data_dir).expect("remove backup fixture");
    }

    #[test]
    fn creates_unique_legacy_db_backup_paths_without_overwriting() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-unique-backup-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create backup fixture dir");

        let first_backup_path = data_dir.join("clipstash.db.bak-20260608-120000");
        fs::write(&first_backup_path, b"first-backup").expect("seed existing backup");

        let next_path = next_backup_path(&data_dir, "20260608-120000");

        assert_eq!(
            next_path.file_name().expect("backup filename"),
            "clipstash.db.bak-20260608-120000-1"
        );
        assert_eq!(
            fs::read(first_backup_path).expect("read existing backup"),
            b"first-backup"
        );

        fs::remove_dir_all(data_dir).expect("remove backup fixture");
    }

    #[test]
    fn creates_text_message_in_temp_legacy_db_after_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-text-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('existing', 0);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let backup = create_legacy_db_backup_for_path(&db_path).expect("create backup first");
        let backup_path = PathBuf::from(&backup.backup_path);
        let message = create_text_message_for_path(&db_path, Some("new text".to_string()))
            .expect("create text message");

        assert!(backup_path.is_file());
        assert_eq!(message.text_content.as_deref(), Some("new text"));
        assert!(!message.archived);
        assert!(message.archived_at.is_none());
        assert!(message.images.is_empty());

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        let message_count = query_count(&conn, "SELECT COUNT(*) FROM messages")
            .expect("count messages after insert");
        let image_count = query_count(&conn, "SELECT COUNT(*) FROM message_images")
            .expect("count images after insert");
        let archived: i64 = conn
            .query_row(
                "SELECT archived FROM messages WHERE id = ?",
                [message.id],
                |row| row.get(0),
            )
            .expect("read archived flag");

        assert_eq!(message_count, 2);
        assert_eq!(image_count, 0);
        assert_eq!(archived, 0);
        drop(conn);

        let page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("read messages after insert");
        assert!(page
            .messages
            .iter()
            .any(|item| item.id == message.id && item.text_content.as_deref() == Some("new text")));

        let backup_conn = Connection::open(&backup_path).expect("open backup sqlite fixture");
        let backup_message_count = query_count(&backup_conn, "SELECT COUNT(*) FROM messages")
            .expect("count backup messages");
        assert_eq!(backup_message_count, 1);
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn creates_text_message_with_backup_wrapper() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-text-wrapper-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('existing', 0);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = create_text_message_with_backup_for_path(
            &db_path,
            "  command text message  ".to_string(),
        )
        .expect("create text message with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        assert_eq!(
            result.message.text_content.as_deref(),
            Some("command text message")
        );
        assert!(!result.message.archived);
        assert!(result.message.images.is_empty());

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            2
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            0
        );
        drop(conn);

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM messages")
                .expect("count backup messages"),
            1
        );
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_blank_text_message_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-blank-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = create_text_message_with_backup_for_path(&db_path, "   ".to_string());

        assert!(result.is_err());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| entry
                .expect("read data dir entry")
                .file_name()
                .to_string_lossy()
                .starts_with("clipstash.db.bak-")));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn updates_text_message_with_backup_and_preserves_images() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-update-text-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).expect("create images dir");
        fs::write(images_dir.join("existing.png"), b"existing-image").expect("write image");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('old text', 0);
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'existing.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = update_text_message_with_backup_for_path(
            &db_path,
            1,
            Some("  updated text  ".to_string()),
        )
        .expect("update text message with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        assert_eq!(result.message.id, 1);
        assert_eq!(result.message.text_content.as_deref(), Some("updated text"));
        assert_eq!(result.message.images.len(), 1);
        assert_eq!(result.message.images[0].filename, "existing.png");
        assert!(result.message.images[0].exists);

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            1
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            1
        );
        let text: Option<String> = conn
            .query_row(
                "SELECT text_content FROM messages WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read updated text");
        assert_eq!(text.as_deref(), Some("updated text"));
        drop(conn);

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        let backup_text: Option<String> = backup_conn
            .query_row(
                "SELECT text_content FROM messages WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("read backup text");
        assert_eq!(backup_text.as_deref(), Some("old text"));
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_missing_text_update_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-update-missing-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result =
            update_text_message_with_backup_for_path(&db_path, 404, Some("new text".to_string()));

        assert!(result.is_err());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| entry
                .expect("read data dir entry")
                .file_name()
                .to_string_lossy()
                .starts_with("clipstash.db.bak-")));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn replaces_message_images_with_db_and_file_backups() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-replace-images-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).expect("create images dir");
        fs::write(images_dir.join("old-a.png"), b"old-a").expect("write old image a");
        fs::write(images_dir.join("old-b.png"), b"old-b").expect("write old image b");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('has text', 0);
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'old-a.png');
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'old-b.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result =
            replace_message_images_with_backup_for_path(&db_path, 1, vec![b"new-image".to_vec()])
                .expect("replace message images with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        let image_backup = result.image_backup.expect("image backup");
        let image_backup_dir = PathBuf::from(&image_backup.backup_dir);
        assert!(image_backup_dir.is_dir());
        assert_eq!(image_backup.filenames, vec!["old-a.png", "old-b.png"]);
        assert_eq!(
            fs::read(image_backup_dir.join("old-a.png")).expect("read old image a backup"),
            b"old-a"
        );
        assert_eq!(
            fs::read(image_backup_dir.join("old-b.png")).expect("read old image b backup"),
            b"old-b"
        );

        assert_eq!(result.message.id, 1);
        assert_eq!(result.message.text_content.as_deref(), Some("has text"));
        assert_eq!(result.message.images.len(), 1);
        assert!(result.message.images[0].exists);
        assert_eq!(
            fs::read(&result.message.images[0].path).expect("read new image"),
            b"new-image"
        );
        assert!(!images_dir.join("old-a.png").exists());
        assert!(!images_dir.join("old-b.png").exists());

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            1
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            1
        );
        let image_rows = query_image_rows(&conn, result.message.id);
        assert_eq!(image_rows.len(), 1);
        assert_eq!(image_rows[0].1, result.message.images[0].filename);
        drop(conn);

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM message_images")
                .expect("count backup images"),
            2
        );
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_clearing_image_only_message_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-clear-image-only-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).expect("create images dir");
        fs::write(images_dir.join("only.png"), b"only-image").expect("write only image");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES (NULL, 0);
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'only.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = replace_message_images_with_backup_for_path(&db_path, 1, Vec::new());

        assert!(result.is_err());
        assert!(images_dir.join("only.png").is_file());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| {
                let name = entry
                    .expect("read data dir entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                name.starts_with("clipstash.db.bak-") || name.starts_with("images.bak-")
            }));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn deletes_message_with_db_and_file_backups() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-delete-message-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).expect("create images dir");
        fs::write(images_dir.join("delete-a.png"), b"delete-a").expect("write image a");
        fs::write(images_dir.join("delete-b.png"), b"delete-b").expect("write image b");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('delete me', 0);
            INSERT INTO messages (text_content, archived) VALUES ('keep me', 0);
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'delete-a.png');
            INSERT INTO message_images (message_id, image_filename) VALUES (1, 'delete-b.png');
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result =
            delete_message_with_backup_for_path(&db_path, 1).expect("delete message with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        assert_eq!(result.message.id, 1);
        assert_eq!(result.message.text_content.as_deref(), Some("delete me"));
        assert_eq!(result.message.images.len(), 2);
        let image_backup = result.image_backup.expect("image backup");
        let image_backup_dir = PathBuf::from(&image_backup.backup_dir);
        assert_eq!(image_backup.filenames, vec!["delete-a.png", "delete-b.png"]);
        assert_eq!(
            fs::read(image_backup_dir.join("delete-a.png")).expect("read image a backup"),
            b"delete-a"
        );
        assert_eq!(
            fs::read(image_backup_dir.join("delete-b.png")).expect("read image b backup"),
            b"delete-b"
        );
        assert!(!images_dir.join("delete-a.png").exists());
        assert!(!images_dir.join("delete-b.png").exists());

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            1
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            0
        );
        let kept_text: String = conn
            .query_row(
                "SELECT text_content FROM messages WHERE id = 2",
                [],
                |row| row.get(0),
            )
            .expect("read kept message");
        assert_eq!(kept_text, "keep me");
        drop(conn);

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM messages")
                .expect("count backup messages"),
            2
        );
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM message_images")
                .expect("count backup images"),
            2
        );
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_missing_delete_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-delete-missing-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = delete_message_with_backup_for_path(&db_path, 404);

        assert!(result.is_err());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| {
                let name = entry
                    .expect("read data dir entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string();
                name.starts_with("clipstash.db.bak-") || name.starts_with("images.bak-")
            }));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn archives_and_restores_message_with_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-archive-message-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived, archived_at)
            VALUES ('toggle me', 0, NULL);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let archived = set_message_archived_with_backup_for_path(&db_path, 1, true)
            .expect("archive message with backup");

        assert!(PathBuf::from(&archived.backup.backup_path).is_file());
        assert!(archived.message.archived);
        assert!(archived.message.archived_at.is_some());

        let archive_backup_conn =
            Connection::open(&archived.backup.backup_path).expect("open archive backup fixture");
        let backup_archived: i64 = archive_backup_conn
            .query_row("SELECT archived FROM messages WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("read archive backup flag");
        let backup_archived_at: Option<String> = archive_backup_conn
            .query_row("SELECT archived_at FROM messages WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("read archive backup timestamp");
        assert_eq!(backup_archived, 0);
        assert!(backup_archived_at.is_none());
        drop(archive_backup_conn);

        let restored = set_message_archived_with_backup_for_path(&db_path, 1, false)
            .expect("restore message with backup");

        assert!(PathBuf::from(&restored.backup.backup_path).is_file());
        assert!(!restored.message.archived);
        assert!(restored.message.archived_at.is_none());

        let page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("list normal messages after restore");
        assert_eq!(page.total_count, 1);
        assert_eq!(page.messages[0].id, 1);
        assert!(!page.messages[0].archived);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_missing_archive_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-archive-missing-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = set_message_archived_with_backup_for_path(&db_path, 404, true);

        assert!(result.is_err());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| entry
                .expect("read data dir entry")
                .file_name()
                .to_string_lossy()
                .starts_with("clipstash.db.bak-")));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn resolves_legacy_image_path_only_for_images_dir_filenames() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-copy-image-path-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).expect("create images dir");
        fs::write(images_dir.join("safe.png"), tiny_png_bytes()).expect("write image fixture");

        let resolved =
            resolve_legacy_image_path(&data_dir, "safe.png").expect("resolve image fixture");

        assert_eq!(resolved, images_dir.join("safe.png"));
        fs::remove_dir_all(data_dir).expect("remove image path fixture");
    }

    #[test]
    fn rejects_legacy_image_path_traversal_before_copy() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-copy-image-traversal-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create images dir");

        let result = resolve_legacy_image_path(&data_dir, "..\\clipstash.db");

        assert!(result.is_err());
        fs::remove_dir_all(data_dir).expect("remove image traversal fixture");
    }

    #[test]
    fn creates_image_message_in_temp_legacy_db_after_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-image-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('existing', 0);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = create_image_message_with_backup_for_path(
            &db_path,
            vec![
                b"first-image-bytes".to_vec(),
                b"second-image-bytes".to_vec(),
            ],
        )
        .expect("create image message with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        assert!(result.message.text_content.is_none());
        assert!(!result.message.archived);
        assert_eq!(result.message.images.len(), 2);
        assert_ne!(
            result.message.images[0].filename,
            result.message.images[1].filename
        );

        let first_image_path = PathBuf::from(&result.message.images[0].path);
        let second_image_path = PathBuf::from(&result.message.images[1].path);
        assert!(first_image_path.is_file());
        assert!(second_image_path.is_file());
        assert_eq!(
            fs::read(first_image_path).expect("read first image"),
            b"first-image-bytes"
        );
        assert_eq!(
            fs::read(second_image_path).expect("read second image"),
            b"second-image-bytes"
        );

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            2
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            2
        );
        let image_rows = query_image_rows(&conn, result.message.id);
        assert_eq!(image_rows.len(), 2);
        assert_eq!(image_rows[0].1, result.message.images[0].filename);
        assert_eq!(image_rows[1].1, result.message.images[1].filename);
        assert!(image_rows[0].0 < image_rows[1].0);
        drop(conn);

        let page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("read messages after image insert");
        let listed = page
            .messages
            .iter()
            .find(|item| item.id == result.message.id)
            .expect("listed image message");
        assert_eq!(listed.images.len(), 2);
        assert!(listed.images.iter().all(|image| image.exists));

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM messages")
                .expect("count backup messages"),
            1
        );
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM message_images")
                .expect("count backup images"),
            0
        );
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn creates_mixed_message_in_temp_legacy_db_after_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-mixed-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
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
            INSERT INTO messages (text_content, archived) VALUES ('existing', 0);
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = create_mixed_message_with_backup_for_path(
            &db_path,
            "  mixed text message  ".to_string(),
            vec![
                b"mixed-first-image".to_vec(),
                b"mixed-second-image".to_vec(),
            ],
        )
        .expect("create mixed message with backup");

        assert!(PathBuf::from(&result.backup.backup_path).is_file());
        assert_eq!(
            result.message.text_content.as_deref(),
            Some("mixed text message")
        );
        assert!(!result.message.archived);
        assert_eq!(result.message.images.len(), 2);

        assert_eq!(
            fs::read(&result.message.images[0].path).expect("read first mixed image"),
            b"mixed-first-image"
        );
        assert_eq!(
            fs::read(&result.message.images[1].path).expect("read second mixed image"),
            b"mixed-second-image"
        );

        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            2
        );
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM message_images").expect("count images"),
            2
        );
        let image_rows = query_image_rows(&conn, result.message.id);
        assert_eq!(image_rows.len(), 2);
        assert_eq!(image_rows[0].1, result.message.images[0].filename);
        assert_eq!(image_rows[1].1, result.message.images[1].filename);
        assert!(image_rows[0].0 < image_rows[1].0);
        drop(conn);

        let page = list_legacy_messages_from_dir(
            data_dir.clone(),
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(10),
        )
        .expect("read messages after mixed insert");
        let listed = page
            .messages
            .iter()
            .find(|item| item.id == result.message.id)
            .expect("listed mixed message");
        assert_eq!(listed.text_content.as_deref(), Some("mixed text message"));
        assert_eq!(listed.images.len(), 2);
        assert!(listed.images.iter().all(|image| image.exists));

        let backup_conn =
            Connection::open(&result.backup.backup_path).expect("open backup sqlite fixture");
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM messages")
                .expect("count backup messages"),
            1
        );
        assert_eq!(
            query_count(&backup_conn, "SELECT COUNT(*) FROM message_images")
                .expect("count backup images"),
            0
        );
        drop(backup_conn);

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn rejects_empty_image_message_before_backup() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-create-empty-image-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            ",
        )
        .expect("seed fixture");
        drop(conn);

        let result = create_image_message_with_backup_for_path(&db_path, Vec::new());

        assert!(result.is_err());
        assert!(!fs::read_dir(&data_dir)
            .expect("read data dir")
            .any(|entry| entry
                .expect("read data dir entry")
                .file_name()
                .to_string_lossy()
                .starts_with("clipstash.db.bak-")));

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    fn removes_saved_image_files_when_db_insert_fails() {
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-image-cleanup-test-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(&data_dir).expect("create data dir");

        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("create sqlite fixture");
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            ",
        )
        .expect("seed fixture without message_images table");
        drop(conn);

        let result = create_image_message_for_path(&db_path, vec![b"orphan-risk".to_vec()]);

        assert!(result.is_err());
        let conn = Connection::open(&db_path).expect("open sqlite fixture");
        assert_eq!(
            query_count(&conn, "SELECT COUNT(*) FROM messages").expect("count messages"),
            0
        );
        drop(conn);

        let images_dir = data_dir.join("images");
        assert!(images_dir.is_dir());
        assert_eq!(
            fs::read_dir(&images_dir).expect("read images dir").count(),
            0
        );

        fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
    }

    #[test]
    #[ignore = "requires local ClipStash app data"]
    fn reads_local_legacy_stats_when_available() {
        let stats = read_legacy_stats().expect("read local legacy stats");

        eprintln!(
            "normal={} archived={} total={} db={}",
            stats.normal_count, stats.archived_count, stats.total_count, stats.db_path
        );

        assert!(stats.db_exists);
        assert_eq!(stats.total_count, stats.normal_count + stats.archived_count);
    }

    #[test]
    #[ignore = "requires local ClipStash app data"]
    fn lists_local_legacy_messages_when_available() {
        let page = list_legacy_messages(MessageView::Normal, SortOrder::Newest, Some(0), Some(5))
            .expect("list local legacy messages");

        eprintln!(
            "view={} total={} returned={} has_more={}",
            page.view,
            page.total_count,
            page.messages.len(),
            page.has_more
        );

        assert!(page.total_count >= page.messages.len() as i64);
        for message in page.messages {
            assert!(!message.archived);
        }
    }

    #[test]
    #[ignore = "requires local ClipStash app data"]
    fn verifies_local_legacy_readonly_consistency() {
        let data_dir = legacy_data_dir().expect("resolve local legacy data dir");
        let db_path = data_dir.join("clipstash.db");
        let images_dir = data_dir.join("images");
        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open local legacy database read-only");

        let stats = read_legacy_stats_from_dir(data_dir.clone()).expect("read local stats");
        let normal_messages = collect_all_messages(data_dir.clone(), MessageView::Normal);
        let archived_messages = collect_all_messages(data_dir.clone(), MessageView::Archived);
        let all_messages: Vec<&LegacyMessage> = normal_messages
            .iter()
            .chain(archived_messages.iter())
            .collect();

        assert_eq!(stats.normal_count, normal_messages.len() as i64);
        assert_eq!(stats.archived_count, archived_messages.len() as i64);
        assert_eq!(
            stats.total_count,
            (normal_messages.len() + archived_messages.len()) as i64
        );

        for message in &normal_messages {
            assert!(
                !message.archived,
                "normal view included archived message {}",
                message.id
            );
        }
        for message in &archived_messages {
            assert!(
                message.archived,
                "archived view included normal message {}",
                message.id
            );
        }

        assert_message_order_matches_db(&conn, MessageView::Normal, SortOrder::Newest);
        assert_message_order_matches_db(&conn, MessageView::Normal, SortOrder::Oldest);
        assert_message_order_matches_db(&conn, MessageView::Archived, SortOrder::Newest);
        assert_message_order_matches_db(&conn, MessageView::Archived, SortOrder::Oldest);

        let api_image_count: i64 = all_messages
            .iter()
            .map(|message| message.images.len() as i64)
            .sum();
        let db_joined_image_count = query_count(
            &conn,
            "SELECT COUNT(*) \
             FROM message_images mi \
             JOIN messages m ON m.id = mi.message_id",
        )
        .expect("count joined images");
        let db_orphan_image_count = query_count(
            &conn,
            "SELECT COUNT(*) \
             FROM message_images mi \
             LEFT JOIN messages m ON m.id = mi.message_id \
             WHERE m.id IS NULL",
        )
        .expect("count orphan images");

        assert_eq!(api_image_count, db_joined_image_count);

        for message in all_messages {
            let db_images = query_image_rows(&conn, message.id);
            assert_eq!(
                db_images.len(),
                message.images.len(),
                "image count mismatch for message {}",
                message.id
            );

            let mut previous_image_id = None;
            for (index, image) in message.images.iter().enumerate() {
                let (db_image_id, db_filename) = &db_images[index];
                assert_eq!(
                    &image.id, db_image_id,
                    "image id mismatch for message {}",
                    message.id
                );
                assert_eq!(
                    &image.filename, db_filename,
                    "image filename mismatch for message {}",
                    message.id
                );
                assert_eq!(
                    image.exists,
                    images_dir.join(&image.filename).is_file(),
                    "image file status mismatch for {}",
                    image.filename
                );

                if let Some(previous) = previous_image_id {
                    assert!(
                        image.id > previous,
                        "image order is not ascending for message {}",
                        message.id
                    );
                }
                previous_image_id = Some(image.id);
            }
        }

        eprintln!(
            "legacy-readonly-ok normal={} archived={} total={} joined_images={} orphan_images={} db={}",
            stats.normal_count,
            stats.archived_count,
            stats.total_count,
            db_joined_image_count,
            db_orphan_image_count,
            db_path.display()
        );
    }

    #[test]
    #[ignore = "writes local ClipStash app data; set CLIPSTASH_NEXT_WRITE_LEGACY_TEXT"]
    fn manual_creates_local_legacy_text_message_with_backup() {
        let text = std::env::var("CLIPSTASH_NEXT_WRITE_LEGACY_TEXT")
            .expect("set CLIPSTASH_NEXT_WRITE_LEGACY_TEXT to the text message to create");
        let result = create_legacy_text_message(text).expect("create local legacy text message");
        let backup_path = PathBuf::from(&result.backup.backup_path);

        assert!(backup_path.is_file());
        assert!(result.backup.bytes_copied > 0);
        assert!(!result.message.archived);
        assert!(result.message.archived_at.is_none());
        assert!(result.message.images.is_empty());

        let latest_page =
            list_legacy_messages(MessageView::Normal, SortOrder::Newest, Some(0), Some(1))
                .expect("list latest local legacy message");
        let latest = latest_page
            .messages
            .first()
            .expect("latest local legacy message");

        assert_eq!(latest.id, result.message.id);
        assert_eq!(latest.text_content, result.message.text_content);

        eprintln!(
            "legacy-write-ok id={} backup={} bytes={} text={}",
            result.message.id,
            result.backup.backup_path,
            result.backup.bytes_copied,
            result.message.text_content.as_deref().unwrap_or("")
        );
    }

    #[test]
    #[ignore = "writes local ClipStash app data; set CLIPSTASH_NEXT_WRITE_LEGACY_IMAGE"]
    fn manual_creates_local_legacy_image_message_with_backup() {
        std::env::var("CLIPSTASH_NEXT_WRITE_LEGACY_IMAGE")
            .expect("set CLIPSTASH_NEXT_WRITE_LEGACY_IMAGE=1 to create a local image message");
        let result = create_legacy_image_message(vec![tiny_png_bytes()])
            .expect("create local image message");
        let backup_path = PathBuf::from(&result.backup.backup_path);

        assert!(backup_path.is_file());
        assert!(result.backup.bytes_copied > 0);
        assert!(result.message.text_content.is_none());
        assert!(!result.message.archived);
        assert!(result.message.archived_at.is_none());
        assert_eq!(result.message.images.len(), 1);
        assert!(result.message.images[0].exists);
        assert!(PathBuf::from(&result.message.images[0].path).is_file());

        let latest_page =
            list_legacy_messages(MessageView::Normal, SortOrder::Newest, Some(0), Some(1))
                .expect("list latest local legacy message");
        let latest = latest_page
            .messages
            .first()
            .expect("latest local legacy message");

        assert_eq!(latest.id, result.message.id);
        assert_eq!(latest.images.len(), 1);
        assert_eq!(latest.images[0].filename, result.message.images[0].filename);

        eprintln!(
            "legacy-image-write-ok id={} image={} path={} backup={} bytes={}",
            result.message.id,
            result.message.images[0].filename,
            result.message.images[0].path,
            result.backup.backup_path,
            result.backup.bytes_copied
        );
    }

    #[test]
    #[ignore = "writes local ClipStash app data; set CLIPSTASH_NEXT_WRITE_LEGACY_MIXED"]
    fn manual_creates_local_legacy_mixed_message_with_backup() {
        let text = std::env::var("CLIPSTASH_NEXT_WRITE_LEGACY_MIXED")
            .expect("set CLIPSTASH_NEXT_WRITE_LEGACY_MIXED to the text message to create");
        let result = create_legacy_mixed_message(text, vec![tiny_png_bytes()])
            .expect("create local mixed message");
        let backup_path = PathBuf::from(&result.backup.backup_path);

        assert!(backup_path.is_file());
        assert!(result.backup.bytes_copied > 0);
        assert_eq!(
            result.message.text_content.as_deref(),
            Some("[ClipStash Next 验收] Tauri 阶段 2 图文混合写入兼容测试 2026-06-08")
        );
        assert!(!result.message.archived);
        assert!(result.message.archived_at.is_none());
        assert_eq!(result.message.images.len(), 1);
        assert!(result.message.images[0].exists);
        assert!(PathBuf::from(&result.message.images[0].path).is_file());

        let latest_page =
            list_legacy_messages(MessageView::Normal, SortOrder::Newest, Some(0), Some(1))
                .expect("list latest local legacy message");
        let latest = latest_page
            .messages
            .first()
            .expect("latest local legacy message");

        assert_eq!(latest.id, result.message.id);
        assert_eq!(latest.text_content, result.message.text_content);
        assert_eq!(latest.images.len(), 1);
        assert_eq!(latest.images[0].filename, result.message.images[0].filename);

        eprintln!(
            "legacy-mixed-write-ok id={} image={} path={} backup={} bytes={} text={}",
            result.message.id,
            result.message.images[0].filename,
            result.message.images[0].path,
            result.backup.backup_path,
            result.backup.bytes_copied,
            result.message.text_content.as_deref().unwrap_or("")
        );
    }

    #[test]
    #[ignore = "writes local ClipStash app data; set CLIPSTASH_NEXT_WRITE_LEGACY_ARCHIVE_ID"]
    fn manual_toggles_local_legacy_archive_with_backup_and_restore() {
        let message_id = std::env::var("CLIPSTASH_NEXT_WRITE_LEGACY_ARCHIVE_ID")
            .expect("set CLIPSTASH_NEXT_WRITE_LEGACY_ARCHIVE_ID to an existing message id")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_WRITE_LEGACY_ARCHIVE_ID must be a positive integer");
        assert!(message_id > 0);

        let data_dir = legacy_data_dir().expect("locate local legacy data dir");
        let db_path = data_dir.join("clipstash.db");
        let original =
            read_message_for_update_precheck(&db_path, message_id).expect("read original message");
        let target_archived = !original.archived;

        let toggled = set_legacy_message_archived(message_id, target_archived)
            .expect("toggle local legacy archive state");
        let toggle_backup_path = PathBuf::from(&toggled.backup.backup_path);

        assert!(toggle_backup_path.is_file());
        assert!(toggled.backup.bytes_copied > 0);
        assert_eq!(toggled.message.id, message_id);
        assert_eq!(toggled.message.archived, target_archived);
        if target_archived {
            assert!(toggled.message.archived_at.is_some());
        } else {
            assert!(toggled.message.archived_at.is_none());
        }

        let restored = set_legacy_message_archived(message_id, original.archived)
            .expect("restore local legacy archive state");
        let restore_backup_path = PathBuf::from(&restored.backup.backup_path);

        assert!(restore_backup_path.is_file());
        assert!(restored.backup.bytes_copied > 0);
        assert_eq!(restored.message.id, message_id);
        assert_eq!(restored.message.archived, original.archived);
        assert_eq!(restored.message.archived_at, original.archived_at);

        eprintln!(
            "legacy-archive-toggle-ok id={} toggled_to={} restored_to={} toggle_backup={} restore_backup={}",
            message_id,
            toggled.message.archived,
            restored.message.archived,
            toggled.backup.backup_path,
            restored.backup.backup_path
        );
    }

    #[test]
    #[ignore = "writes local ClipStash app data; set CLIPSTASH_NEXT_SET_LEGACY_ARCHIVE_ID and CLIPSTASH_NEXT_SET_LEGACY_ARCHIVED"]
    fn manual_sets_local_legacy_archive_state_with_backup() {
        let message_id = std::env::var("CLIPSTASH_NEXT_SET_LEGACY_ARCHIVE_ID")
            .expect("set CLIPSTASH_NEXT_SET_LEGACY_ARCHIVE_ID to an existing message id")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_SET_LEGACY_ARCHIVE_ID must be a positive integer");
        assert!(message_id > 0);

        let archived_value = std::env::var("CLIPSTASH_NEXT_SET_LEGACY_ARCHIVED")
            .expect("set CLIPSTASH_NEXT_SET_LEGACY_ARCHIVED to 0 or 1");
        let archived = match archived_value.as_str() {
            "0" | "false" | "False" | "FALSE" => false,
            "1" | "true" | "True" | "TRUE" => true,
            _ => panic!("CLIPSTASH_NEXT_SET_LEGACY_ARCHIVED must be 0, 1, false, or true"),
        };

        let result = set_legacy_message_archived(message_id, archived)
            .expect("set local legacy archive state");
        let backup_path = PathBuf::from(&result.backup.backup_path);

        assert!(backup_path.is_file());
        assert!(result.backup.bytes_copied > 0);
        assert_eq!(result.message.id, message_id);
        assert_eq!(result.message.archived, archived);
        if archived {
            assert!(result.message.archived_at.is_some());
        } else {
            assert!(result.message.archived_at.is_none());
        }

        eprintln!(
            "legacy-archive-set-ok id={} archived={} backup={}",
            message_id, result.message.archived, result.backup.backup_path
        );
    }

    #[test]
    #[ignore = "writes system clipboard; set CLIPSTASH_NEXT_COPY_LEGACY_IMAGE_FILENAME"]
    fn manual_copies_local_legacy_image_to_system_clipboard() {
        let filename = std::env::var("CLIPSTASH_NEXT_COPY_LEGACY_IMAGE_FILENAME")
            .expect("set CLIPSTASH_NEXT_COPY_LEGACY_IMAGE_FILENAME to an existing image filename");

        let result = copy_legacy_image_to_clipboard(filename).expect("copy local legacy image");

        assert!(PathBuf::from(&result.path).is_file());
        assert!(result.width > 0);
        assert!(result.height > 0);

        eprintln!(
            "legacy-image-copy-ok filename={} width={} height={} path={}",
            result.filename, result.width, result.height, result.path
        );
    }

    #[test]
    #[ignore = "writes system clipboard; set CLIPSTASH_NEXT_STAGE_LEGACY_IMPORT_ID"]
    fn manual_stages_local_legacy_message_import_to_system_clipboard() {
        let message_id = std::env::var("CLIPSTASH_NEXT_STAGE_LEGACY_IMPORT_ID")
            .expect("set CLIPSTASH_NEXT_STAGE_LEGACY_IMPORT_ID to an existing message id")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_STAGE_LEGACY_IMPORT_ID must be an integer");

        let result = stage_legacy_message_import_to_clipboard(message_id)
            .expect("stage local legacy message import");

        assert_eq!(result.message_id, message_id);
        assert!(result.staged_kind == "text" || result.staged_kind == "image");
        if result.staged_kind == "text" {
            assert!(result.text_length > 0);
            assert!(result.copied_image.is_none());
        } else {
            assert!(result.first_image_filename.is_some());
            assert!(result.copied_image.is_some());
        }

        eprintln!(
            "legacy-import-stage-ok id={} kind={} text_length={} image_count={} first_image={:?}",
            result.message_id,
            result.staged_kind,
            result.text_length,
            result.image_count,
            result.first_image_filename
        );
    }

    #[test]
    #[ignore = "writes system clipboard; set CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_ID and CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_INDEX"]
    fn manual_copies_local_legacy_import_queue_item_to_system_clipboard() {
        let message_id = std::env::var("CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_ID")
            .expect("set CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_ID to an existing message id")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_ID must be an integer");
        let item_index = std::env::var("CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_INDEX")
            .expect("set CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_INDEX to a queue item index")
            .parse::<usize>()
            .expect("CLIPSTASH_NEXT_COPY_LEGACY_IMPORT_INDEX must be a zero-based integer");

        let result = copy_legacy_message_import_queue_item_to_clipboard(message_id, item_index)
            .expect("copy local legacy import queue item");

        assert_eq!(result.message_id, message_id);
        assert_eq!(result.item_index, item_index);
        assert!(result.staged_kind == "text" || result.staged_kind == "image");
        if result.staged_kind == "text" {
            assert!(result.text_length > 0);
            assert!(result.image_filename.is_none());
            assert!(result.copied_image.is_none());
        } else {
            assert!(result.image_filename.is_some());
            assert!(result.copied_image.is_some());
        }

        eprintln!(
            "legacy-import-queue-copy-ok id={} index={} kind={} text_length={} image={:?}",
            result.message_id,
            result.item_index,
            result.staged_kind,
            result.text_length,
            result.image_filename
        );
    }

    fn tiny_png_bytes() -> Vec<u8> {
        vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ]
    }

    fn collect_all_messages(data_dir: PathBuf, view: MessageView) -> Vec<LegacyMessage> {
        let mut offset = 0;
        let mut messages = Vec::new();

        loop {
            let page = list_legacy_messages_from_dir(
                data_dir.clone(),
                view,
                SortOrder::Newest,
                Some(offset),
                Some(17),
            )
            .expect("list legacy messages page");
            offset += page.messages.len() as i64;
            messages.extend(page.messages);

            if !page.has_more {
                break;
            }
        }

        messages
    }

    fn assert_message_order_matches_db(conn: &Connection, view: MessageView, sort: SortOrder) {
        let data_dir = legacy_data_dir().expect("resolve local legacy data dir");
        let api_ids: Vec<i64> = collect_all_messages_with_sort(data_dir, view, sort)
            .iter()
            .map(|message| message.id)
            .collect();
        let db_ids = query_message_ids(conn, view, sort);

        assert_eq!(api_ids, db_ids);
    }

    fn collect_all_messages_with_sort(
        data_dir: PathBuf,
        view: MessageView,
        sort: SortOrder,
    ) -> Vec<LegacyMessage> {
        let mut offset = 0;
        let mut messages = Vec::new();

        loop {
            let page =
                list_legacy_messages_from_dir(data_dir.clone(), view, sort, Some(offset), Some(17))
                    .expect("list sorted legacy messages page");
            offset += page.messages.len() as i64;
            messages.extend(page.messages);

            if !page.has_more {
                break;
            }
        }

        messages
    }

    fn query_message_ids(conn: &Connection, view: MessageView, sort: SortOrder) -> Vec<i64> {
        let order = match sort {
            SortOrder::Newest => "DESC",
            SortOrder::Oldest => "ASC",
        };
        let sort_column = match view {
            MessageView::Normal => "created_at",
            MessageView::Archived => "COALESCE(archived_at, created_at)",
        };
        let sql = format!(
            "SELECT id FROM messages WHERE {} ORDER BY {sort_column} {order}, id {order}",
            view_where_sql(view)
        );
        let mut stmt = conn.prepare(&sql).expect("prepare message id query");
        stmt.query_map([], |row| row.get::<_, i64>(0))
            .expect("query message ids")
            .map(|row| row.expect("read message id"))
            .collect()
    }

    fn query_image_rows(conn: &Connection, message_id: i64) -> Vec<(i64, String)> {
        let mut stmt = conn
            .prepare(
                "SELECT id, image_filename \
                 FROM message_images \
                 WHERE message_id = ? \
                 ORDER BY id",
            )
            .expect("prepare image row query");
        stmt.query_map([message_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query image rows")
        .map(|row| row.expect("read image row"))
        .collect()
    }
}
