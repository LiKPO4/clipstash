use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::{env, path::PathBuf};

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

pub fn read_legacy_stats() -> Result<LegacyStats, String> {
    let data_dir = legacy_data_dir()?;
    read_legacy_stats_from_dir(data_dir)
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

fn query_count(conn: &Connection, sql: &str) -> Result<i64, String> {
    conn.query_row(sql, [], |row| row.get(0))
        .map_err(|err| format!("查询旧数据库计数失败：{err}"))
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
}
