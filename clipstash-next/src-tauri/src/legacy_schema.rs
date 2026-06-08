use rusqlite::Connection;

pub(crate) fn ensure_legacy_schema(conn: &Connection) -> Result<(), String> {
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
