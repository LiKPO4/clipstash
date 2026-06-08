use crate::{
    legacy_model::LegacyMessage, legacy_query::read_legacy_message_by_id,
    legacy_schema::ensure_legacy_schema,
};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub(crate) fn validate_replace_images_request(
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

pub(crate) fn ensure_message_exists_for_path(
    db_path: &Path,
    message_id: i64,
) -> Result<(), String> {
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

pub(crate) fn read_message_for_update_precheck(
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
