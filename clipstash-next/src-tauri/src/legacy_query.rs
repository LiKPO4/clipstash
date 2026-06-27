use crate::{
    legacy_model::{LegacyMessage, LegacyMessageImage, LegacyMessagePage, MessageView, SortOrder},
    legacy_paths::path_to_string,
    legacy_schema::ensure_legacy_schema,
};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use std::path::PathBuf;

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

pub(crate) fn read_legacy_stats_from_dir(data_dir: PathBuf) -> Result<LegacyStats, String> {
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
        data_dir: path_to_string(&data_dir),
        db_path: path_to_string(&db_path),
        images_dir: path_to_string(&images_dir),
        db_exists,
        images_dir_exists,
        normal_count,
        archived_count,
        total_count,
    })
}

pub(crate) fn query_count(conn: &Connection, sql: &str) -> Result<i64, String> {
    conn.query_row(sql, [], |row| row.get(0))
        .map_err(|err| format!("查询旧数据库计数失败：{err}"))
}

pub(crate) fn list_legacy_messages_from_dir(
    data_dir: PathBuf,
    view: MessageView,
    sort: SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<LegacyMessagePage, String> {
    list_legacy_messages_from_dir_filtered(data_dir, view, sort, offset, limit, None)
}

pub(crate) fn list_legacy_messages_from_dir_filtered(
    data_dir: PathBuf,
    view: MessageView,
    sort: SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
    search: Option<String>,
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
    let normalized_search = search
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let search_pattern = normalized_search
        .as_ref()
        .map(|value| format!("%{}%", value));
    let total_count = if let Some(pattern) = search_pattern.as_deref() {
        conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM messages WHERE ({}) AND text_content LIKE ?",
                view_where_sql(view)
            ),
            params![pattern],
            |row| row.get(0),
        )
        .map_err(|err| format!("查询旧数据库计数失败：{err}"))?
    } else {
        query_count(&conn, view_count_sql(view))?
    };
    let order = match sort {
        SortOrder::Newest => "DESC",
        SortOrder::Oldest => "ASC",
    };
    let sort_column = match view {
        MessageView::Normal => "created_at",
        MessageView::Archived => "COALESCE(archived_at, created_at)",
    };
    let where_sql = if search_pattern.is_some() {
        format!("({}) AND text_content LIKE ?", view_where_sql(view))
    } else {
        view_where_sql(view).to_string()
    };
    let sql = format!(
        "SELECT id, text_content, created_at, archived, archived_at \
         FROM messages \
         WHERE {where_sql} \
         ORDER BY {sort_column} {order}, id {order} \
         LIMIT ? OFFSET ?"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("准备旧消息查询失败：{err}"))?;
    let map_row = |row: &rusqlite::Row<'_>| {
        let archived: i64 = row.get(3)?;
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            archived == 1,
            row.get::<_, Option<String>>(4)?,
        ))
    };
    let rows = if let Some(pattern) = search_pattern.as_deref() {
        stmt.query_map(params![pattern, limit, offset], map_row)
    } else {
        stmt.query_map(params![limit, offset], map_row)
    }
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

pub(crate) fn view_where_sql(view: MessageView) -> &'static str {
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

#[allow(dead_code)]
pub(crate) fn read_legacy_message_by_id(
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
