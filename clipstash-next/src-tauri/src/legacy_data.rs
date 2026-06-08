use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};

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

pub fn read_legacy_stats() -> Result<LegacyStats, String> {
    let data_dir = legacy_data_dir()?;
    read_legacy_stats_from_dir(data_dir)
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
        let data_dir = env::temp_dir().join(format!(
            "clipstash-next-legacy-list-test-{}",
            process::id()
        ));
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
        let page = list_legacy_messages(
            MessageView::Normal,
            SortOrder::Newest,
            Some(0),
            Some(5),
        )
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
        let all_messages: Vec<&LegacyMessage> =
            normal_messages.iter().chain(archived_messages.iter()).collect();

        assert_eq!(stats.normal_count, normal_messages.len() as i64);
        assert_eq!(stats.archived_count, archived_messages.len() as i64);
        assert_eq!(
            stats.total_count,
            (normal_messages.len() + archived_messages.len()) as i64
        );

        for message in &normal_messages {
            assert!(!message.archived, "normal view included archived message {}", message.id);
        }
        for message in &archived_messages {
            assert!(message.archived, "archived view included normal message {}", message.id);
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
                assert_eq!(&image.id, db_image_id, "image id mismatch for message {}", message.id);
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
            let page = list_legacy_messages_from_dir(
                data_dir.clone(),
                view,
                sort,
                Some(offset),
                Some(17),
            )
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
