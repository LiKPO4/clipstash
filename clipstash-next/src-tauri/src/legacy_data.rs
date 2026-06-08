use crate::legacy_backup::create_legacy_db_backup_for_path;
#[cfg(test)]
use crate::legacy_backup::next_backup_path;
pub use crate::legacy_backup::{LegacyDbBackup, LegacyImageFilesBackup};
use crate::legacy_clipboard::{
    copy_legacy_image_to_clipboard_from_dir,
    copy_legacy_message_import_queue_item_to_clipboard_from_dir,
    copy_legacy_message_text_to_clipboard_from_dir, preview_legacy_message_import_queue_from_dir,
    stage_legacy_message_import_to_clipboard_from_dir,
};
pub use crate::legacy_clipboard::{
    LegacyCopyImageResult, LegacyCopyTextResult, LegacyImportQueueCopyResult,
    LegacyImportQueuePreview, LegacyImportStageResult,
};
#[allow(unused_imports)]
pub use crate::legacy_model::LegacyMessageImage;
pub use crate::legacy_model::{LegacyMessage, LegacyMessagePage, MessageView, SortOrder};
use crate::legacy_paths::legacy_data_dir;
#[cfg(test)]
use crate::legacy_query::query_count;
pub use crate::legacy_query::LegacyStats;
use crate::legacy_query::{list_legacy_messages_from_dir, read_legacy_stats_from_dir};
pub use crate::legacy_write_audit::LegacyWriteAudit;
use crate::legacy_write_ops::{
    create_image_message_with_backup_for_path, create_mixed_message_with_backup_for_path,
    create_text_message_with_backup_for_path, delete_message_with_backup_for_path,
    replace_message_images_with_backup_for_path, set_message_archived_with_backup_for_path,
    update_text_message_with_backup_for_path,
};
#[cfg(test)]
use rusqlite::OpenFlags;
use serde::Serialize;

#[derive(Serialize)]
pub struct LegacyCreateTextMessageResult {
    pub backup: LegacyDbBackup,
    pub audit: LegacyWriteAudit,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyUpdateMessageResult {
    pub backup: LegacyDbBackup,
    pub audit: LegacyWriteAudit,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyReplaceImagesResult {
    pub backup: LegacyDbBackup,
    pub audit: LegacyWriteAudit,
    pub image_backup: Option<LegacyImageFilesBackup>,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyDeleteMessageResult {
    pub backup: LegacyDbBackup,
    pub audit: LegacyWriteAudit,
    pub image_backup: Option<LegacyImageFilesBackup>,
    pub message: LegacyMessage,
}

#[derive(Serialize)]
pub struct LegacyArchiveMessageResult {
    pub backup: LegacyDbBackup,
    pub audit: LegacyWriteAudit,
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

pub fn copy_legacy_message_text_to_clipboard(
    message_id: i64,
) -> Result<LegacyCopyTextResult, String> {
    let data_dir = legacy_data_dir()?;
    copy_legacy_message_text_to_clipboard_from_dir(data_dir, message_id)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_image_files::resolve_legacy_image_path;
    use crate::legacy_test_support::{
        assert_message_order_matches_db, collect_all_messages, query_image_rows, tiny_png_bytes,
    };
    use crate::legacy_write_exec::{create_image_message_for_path, create_text_message_for_path};
    use crate::legacy_write_precheck::read_message_for_update_precheck;
    use rusqlite::Connection;
    use std::{env, fs, path::PathBuf, process};

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
        assert_eq!(result.audit.operation, "create_text_message");
        assert_eq!(result.audit.message_id, result.message.id);
        assert_eq!(result.audit.db_backup_path, result.backup.backup_path);
        assert!(result.audit.image_backup_dir.is_none());
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
        assert_eq!(result.audit.operation, "update_message_text");
        assert_eq!(result.audit.message_id, result.message.id);
        assert_eq!(result.audit.db_backup_path, result.backup.backup_path);
        assert!(result.audit.image_backup_dir.is_none());
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
        assert_eq!(result.audit.operation, "replace_message_images");
        assert_eq!(result.audit.message_id, result.message.id);
        assert_eq!(result.audit.db_backup_path, result.backup.backup_path);
        let image_backup = result.image_backup.expect("image backup");
        assert_eq!(
            result.audit.image_backup_dir.as_deref(),
            Some(image_backup.backup_dir.as_str())
        );
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
        assert_eq!(result.audit.operation, "delete_message");
        assert_eq!(result.audit.message_id, result.message.id);
        assert_eq!(result.audit.db_backup_path, result.backup.backup_path);
        assert_eq!(result.message.id, 1);
        assert_eq!(result.message.text_content.as_deref(), Some("delete me"));
        assert_eq!(result.message.images.len(), 2);
        let image_backup = result.image_backup.expect("image backup");
        assert_eq!(
            result.audit.image_backup_dir.as_deref(),
            Some(image_backup.backup_dir.as_str())
        );
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
        assert_eq!(archived.audit.operation, "set_message_archived");
        assert_eq!(archived.audit.message_id, archived.message.id);
        assert_eq!(archived.audit.db_backup_path, archived.backup.backup_path);
        assert!(archived.audit.image_backup_dir.is_none());
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
}
