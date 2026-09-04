use crate::{
    legacy_model::{LegacyMessage, LegacyMessageImage, LegacyMessagePage, MessageView, SortOrder},
    legacy_paths::path_to_string,
    legacy_schema::{configure_connection, ensure_legacy_schema},
};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf};

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
    configure_connection(&conn)?;

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
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;
    // Keep page rows, associated images, and any exact count in one read snapshot.
    let snapshot = conn
        .unchecked_transaction()
        .map_err(|err| format!("开始消息读取事务失败：{err}"))?;

    let offset = offset.unwrap_or(0).max(0);
    let limit = limit
        .unwrap_or(DEFAULT_MESSAGE_LIMIT)
        .clamp(1, MAX_MESSAGE_LIMIT);
    let normalized_search = search
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let search_pattern = normalized_search
        .as_ref()
        .map(|value| format!("%{}%", escape_like_pattern(value)));
    let order = match sort {
        SortOrder::Newest => "DESC",
        SortOrder::Oldest => "ASC",
    };
    let sort_column = match view {
        MessageView::Normal => "created_at",
        MessageView::Archived => "COALESCE(archived_at, created_at)",
    };
    let where_sql = if search_pattern.is_some() {
        format!(
            "({}) AND text_content LIKE ? ESCAPE '\\'",
            view_where_sql(view)
        )
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
        let archived: Option<i64> = row.get(3)?;
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            archived == Some(1),
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
        messages.push(LegacyMessage {
            id,
            text_content,
            created_at,
            archived,
            archived_at,
            images: Vec::new(),
        });
    }
    attach_images_for_page(&conn, &images_dir, &mut messages)?;
    let total_count = page_total_count(offset, limit, messages.len(), || {
        if let Some(pattern) = search_pattern.as_deref() {
            conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM messages WHERE ({}) AND text_content LIKE ? ESCAPE '\\'",
                    view_where_sql(view)
                ),
                params![pattern],
                |row| row.get(0),
            )
            .map_err(|err| format!("查询旧数据库计数失败：{err}"))
        } else {
            query_count(&conn, view_count_sql(view))
        }
    })?;

    let has_more = offset + (messages.len() as i64) < total_count;
    snapshot
        .commit()
        .map_err(|err| format!("结束消息读取事务失败：{err}"))?;

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

fn page_total_count(
    offset: i64,
    limit: i64,
    length: usize,
    count: impl FnOnce() -> Result<i64, String>,
) -> Result<i64, String> {
    // A short nonempty page (or first empty page) proves the end in this snapshot.
    // An empty deep page may have overshot after deletions, so still count it.
    if (length as i64) < limit && (offset == 0 || length > 0) {
        Ok(offset + length as i64)
    } else {
        count()
    }
}

#[cfg(test)]
mod count_tests {
    #[test]
    fn skips_count_only_when_page_proves_exact_total() {
        for (offset, length, expected) in [(0, 0, 0), (0, 29, 29), (60, 12, 72)] {
            assert_eq!(
                super::page_total_count(offset, 30, length, || panic!("unneeded COUNT")).unwrap(),
                expected
            );
        }
        assert_eq!(super::page_total_count(0, 30, 30, || Ok(90)).unwrap(), 90);
        assert_eq!(super::page_total_count(90, 30, 0, || Ok(42)).unwrap(), 42);
        assert!(super::page_total_count(0, 30, 30, || Err("read failure".into())).is_err());
    }
}

pub(crate) fn view_where_sql(view: MessageView) -> &'static str {
    match view {
        MessageView::Normal => "COALESCE(archived, 0) = 0",
        MessageView::Archived => "archived = 1",
    }
}

fn view_count_sql(view: MessageView) -> &'static str {
    match view {
        MessageView::Normal => "SELECT COUNT(*) FROM messages WHERE COALESCE(archived, 0) = 0",
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

/// 转义 LIKE 通配符，使搜索按字面量匹配 `\`、`%`、`_`。
fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            _ => escaped.push(ch),
        }
    }
    escaped
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
                let archived: Option<i64> = row.get(3)?;
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    archived == Some(1),
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

// A page has at most 100 messages, safely below SQLite's bound-parameter limit.
fn attach_images_for_page(
    conn: &Connection,
    images_dir: &PathBuf,
    messages: &mut [LegacyMessage],
) -> Result<(), String> {
    if messages.is_empty() {
        return Ok(());
    }
    let positions: HashMap<i64, usize> = messages
        .iter()
        .enumerate()
        .map(|(index, message)| (message.id, index))
        .collect();
    let placeholders = vec!["?"; messages.len()].join(",");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, message_id, image_filename FROM message_images \
             WHERE message_id IN ({placeholders}) ORDER BY message_id, id"
        ))
        .map_err(|err| format!("准备旧图片查询失败：{err}"))?;
    let ids: Vec<i64> = messages.iter().map(|message| message.id).collect();
    let rows = stmt
        .query_map(rusqlite::params_from_iter(ids), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|err| format!("查询旧图片失败：{err}"))?;
    for row in rows {
        let (id, message_id, filename) = row.map_err(|err| format!("读取旧图片行失败：{err}"))?;
        let path = images_dir.join(&filename);
        if let Some(&index) = positions.get(&message_id) {
            messages[index].images.push(LegacyMessageImage {
                id,
                filename,
                exists: path.is_file(),
                path: path_to_string(path),
            });
        }
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
