use chrono::Local;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
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

#[derive(Serialize)]
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

#[allow(dead_code)]
pub fn create_image_message_for_path(
    db_path: &Path,
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
            "INSERT INTO messages (text_content, archived) VALUES (NULL, 0)",
            [],
        )
        .map_err(|err| format!("新增图片消息失败：{err}"))?;

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

fn validate_images_data(images_data: &[Vec<u8>]) -> Result<(), String> {
    if images_data.is_empty() {
        return Err("新增图片消息失败，至少需要一张图片".to_string());
    }
    if images_data.iter().any(|image_data| image_data.is_empty()) {
        return Err("新增图片消息失败，图片数据不能为空".to_string());
    }

    Ok(())
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
