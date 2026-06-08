use crate::{
    legacy_model::{LegacyMessage, MessageView, SortOrder},
    legacy_paths::legacy_data_dir,
    legacy_query::{list_legacy_messages_from_dir, view_where_sql},
};
use rusqlite::Connection;
use std::path::PathBuf;

pub(crate) fn tiny_png_bytes() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

pub(crate) fn collect_all_messages(data_dir: PathBuf, view: MessageView) -> Vec<LegacyMessage> {
    collect_all_messages_with_sort(data_dir, view, SortOrder::Newest)
}

pub(crate) fn assert_message_order_matches_db(
    conn: &Connection,
    view: MessageView,
    sort: SortOrder,
) {
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

pub(crate) fn query_image_rows(conn: &Connection, message_id: i64) -> Vec<(i64, String)> {
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
