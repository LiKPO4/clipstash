use rusqlite::Connection;
use std::time::Duration;

/// 每个新打开的数据库连接都必须调用：设置等待锁释放的超时，
/// 避免长事务（旧库迁移、zip 导入）期间其他连接立即报 SQLITE_BUSY。
/// busy_timeout 是连接级设置，不能像 journal_mode 那样持久化。
pub(crate) fn configure_connection(conn: &Connection) -> Result<(), String> {
    conn.busy_timeout(Duration::from_millis(5000))
        .map_err(|err| format!("设置数据库繁忙等待超时失败：{err}"))
}

/// 校验旧库结构完整：messages 表必须存在且含 created_at/archived/archived_at 列，
/// message_images 表必须存在。缺列或缺表说明这是旧版未迁移的库，
/// 提前返回清晰错误，避免后续读写报出难懂的表/列不存在错误。
pub(crate) fn ensure_legacy_schema(conn: &Connection) -> Result<(), String> {
    let messages_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'messages'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("检查 messages 表失败：{err}"))?;

    if messages_exists == 0 {
        return Err("旧数据库缺少 messages 表，请先在旧版 ClipStash 中完成数据迁移".to_string());
    }

    let required_columns = ["created_at", "archived", "archived_at"];
    let column_names = read_table_columns(conn, "messages")?;
    let missing_columns: Vec<&str> = required_columns
        .iter()
        .copied()
        .filter(|column| !column_names.iter().any(|name| name == column))
        .collect();
    if !missing_columns.is_empty() {
        return Err(format!(
            "旧数据库 messages 表缺少列（{}），这是旧版未迁移的库，请先在旧版 ClipStash 中完成数据迁移",
            missing_columns.join("、")
        ));
    }

    let message_images_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'message_images'",
            [],
            |row| row.get(0),
        )
        .map_err(|err| format!("检查 message_images 表失败：{err}"))?;
    if message_images_exists == 0 {
        return Err("旧数据库缺少 message_images 表，这是旧版未迁移的库，请先在旧版 ClipStash 中完成数据迁移".to_string());
    }

    Ok(())
}

fn read_table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| format!("读取 {table} 表结构失败：{err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("读取 {table} 表结构失败：{err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("读取 {table} 表结构失败：{err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn memory_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    fn full_schema() -> &'static str {
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
        "
    }

    #[test]
    fn accepts_complete_legacy_schema() {
        let conn = memory_conn();
        conn.execute_batch(full_schema()).unwrap();
        ensure_legacy_schema(&conn).unwrap();
    }

    #[test]
    fn rejects_missing_messages_table() {
        let conn = memory_conn();
        let err = ensure_legacy_schema(&conn).unwrap_err();
        assert!(err.contains("缺少 messages 表"), "{err}");
        assert!(err.contains("迁移"), "{err}");
    }

    #[test]
    fn rejects_missing_created_at_column() {
        let conn = memory_conn();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
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
        .unwrap();
        let err = ensure_legacy_schema(&conn).unwrap_err();
        assert!(err.contains("created_at"), "{err}");
        assert!(err.contains("未迁移"), "{err}");
    }

    #[test]
    fn rejects_missing_archived_columns() {
        let conn = memory_conn();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE message_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                image_filename TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        let err = ensure_legacy_schema(&conn).unwrap_err();
        assert!(err.contains("archived"), "{err}");
        assert!(err.contains("archived_at"), "{err}");
        assert!(err.contains("未迁移"), "{err}");
    }

    #[test]
    fn rejects_missing_message_images_table() {
        let conn = memory_conn();
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
        .unwrap();
        let err = ensure_legacy_schema(&conn).unwrap_err();
        assert!(err.contains("message_images"), "{err}");
        assert!(err.contains("未迁移"), "{err}");
    }
}
