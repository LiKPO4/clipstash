use crate::{
    legacy_image_files::{next_image_filename, remove_old_message_image_files, save_image_file},
    legacy_model::LegacyMessage,
    legacy_query::read_legacy_message_by_id,
    legacy_schema::ensure_legacy_schema,
    legacy_write_precheck::validate_replace_images_request,
    legacy_write_validation::validate_images_data,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::{fs, path::Path};

pub(crate) fn create_text_message_for_path(
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

pub(crate) fn split_message_for_path(
    db_path: &Path,
    message_id: i64,
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<Vec<LegacyMessage>, String> {
    if message_id <= 0 {
        return Err("拆分消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("拆分消息失败，数据库不存在：{}", db_path.display()));
    }

    let lines = text_content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.len() < 2 {
        return Err("拆分消息失败，至少需要两行非空文字".to_string());
    }
    if images_data.iter().any(|image| image.is_empty()) {
        return Err("拆分消息失败，图片数据不能为空".to_string());
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("拆分消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|err| format!("创建图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开数据库准备拆分失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let mut saved_images = Vec::new();
    for (index, image_data) in images_data.iter().enumerate() {
        let filename = next_image_filename(&images_dir, index);
        let path = images_dir.join(&filename);
        if let Err(err) = save_image_file(&path, image_data) {
            for (_, saved_path) in &saved_images {
                let _ = fs::remove_file(saved_path);
            }
            return Err(err);
        }
        saved_images.push((filename, path));
    }

    let output_count = lines.len().max(saved_images.len());
    let split_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启消息拆分事务失败：{err}"))?;
        let mut message_ids = Vec::with_capacity(output_count);

        for output_index in (0..output_count).rev() {
            let text = lines.get(output_index).map(String::as_str);
            tx.execute(
                "INSERT INTO messages (text_content, created_at, archived, archived_at) VALUES (?, ?, ?, ?)",
                params![
                    text,
                    old_message.created_at,
                    if old_message.archived { 1 } else { 0 },
                    old_message.archived_at,
                ],
            )
            .map_err(|err| format!("写入拆分消息失败：{err}"))?;
            let new_message_id = tx.last_insert_rowid();
            if let Some((filename, _)) = saved_images.get(output_index) {
                tx.execute(
                    "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                    params![new_message_id, filename],
                )
                .map_err(|err| format!("关联拆分消息图片失败：{err}"))?;
            }
            message_ids.push(new_message_id);
        }

        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![message_id],
        )
        .map_err(|err| format!("删除原消息图片关联失败：{err}"))?;
        let deleted = tx
            .execute("DELETE FROM messages WHERE id = ?", params![message_id])
            .map_err(|err| format!("删除原消息失败：{err}"))?;
        if deleted == 0 {
            return Err(format!("拆分消息失败，原消息不存在：{message_id}"));
        }

        tx.commit()
            .map_err(|err| format!("提交消息拆分事务失败：{err}"))?;
        message_ids.reverse();
        Ok::<Vec<i64>, String>(message_ids)
    })();

    let message_ids = match split_result {
        Ok(ids) => ids,
        Err(err) => {
            for (_, path) in saved_images {
                let _ = fs::remove_file(path);
            }
            return Err(err);
        }
    };

    remove_old_message_image_files(&old_message.images);
    message_ids
        .into_iter()
        .map(|id| read_legacy_message_by_id(&conn, &images_dir, id))
        .collect()
}

pub(crate) fn replace_message_images_for_path(
    db_path: &Path,
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyMessage, String> {
    validate_replace_images_request(db_path, message_id, &images_data)?;

    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "替换消息图片失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let images_dir = data_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|err| format!("创建旧图片目录失败：{err}"))?;

    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备替换图片失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let mut saved_paths = Vec::new();
    let replace_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启图片替换事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![message_id],
        )
        .map_err(|err| format!("删除旧图片关联失败：{err}"))?;

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
            .map_err(|err| format!("提交图片替换失败：{err}"))
    })();

    if let Err(err) = replace_result {
        for path in saved_paths {
            let _ = fs::remove_file(path);
        }
        return Err(err);
    }

    remove_old_message_image_files(&old_message.images);
    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub(crate) fn delete_message_for_path(
    db_path: &Path,
    message_id: i64,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("删除消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("删除消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("删除消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备删除失败：{err}"))?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let delete_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启删除消息事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![message_id],
        )
        .map_err(|err| format!("删除图片关联失败：{err}"))?;
        let deleted = tx
            .execute("DELETE FROM messages WHERE id = ?", params![message_id])
            .map_err(|err| format!("删除消息失败：{err}"))?;
        if deleted == 0 {
            return Err(format!("删除消息失败，消息不存在：{message_id}"));
        }
        tx.commit().map_err(|err| format!("提交删除失败：{err}"))
    })();

    delete_result?;
    remove_old_message_image_files(&old_message.images);
    Ok(old_message)
}

pub(crate) fn set_message_archived_for_path(
    db_path: &Path,
    message_id: i64,
    archived: bool,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("更新归档状态失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!(
            "更新归档状态失败，数据库不存在：{}",
            db_path.display()
        ));
    }

    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "更新归档状态失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let images_dir = data_dir.join("images");
    let conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备更新归档失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let archived_value = if archived { 1 } else { 0 };
    let archived_at = if archived {
        Some(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
    } else {
        None
    };
    let updated = conn
        .execute(
            "UPDATE messages SET archived = ?, archived_at = ? WHERE id = ?",
            params![archived_value, archived_at, message_id],
        )
        .map_err(|err| format!("更新归档状态失败：{err}"))?;

    if updated == 0 {
        return Err(format!("更新归档状态失败，消息不存在：{message_id}"));
    }

    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub(crate) fn update_text_message_for_path(
    db_path: &Path,
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyMessage, String> {
    if message_id <= 0 {
        return Err("更新消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("更新消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("更新消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let conn =
        Connection::open(db_path).map_err(|err| format!("打开旧数据库准备更新失败：{err}"))?;
    ensure_legacy_schema(&conn)?;

    let updated = conn
        .execute(
            "UPDATE messages SET text_content = ? WHERE id = ?",
            params![text_content, message_id],
        )
        .map_err(|err| format!("更新消息文字失败：{err}"))?;

    if updated == 0 {
        return Err(format!("更新消息失败，消息不存在：{message_id}"));
    }

    read_legacy_message_by_id(&conn, &images_dir, message_id)
}

pub(crate) fn create_image_message_for_path(
    db_path: &Path,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyMessage, String> {
    create_mixed_message_for_path(db_path, None, images_data)
}

pub(crate) fn create_mixed_message_for_path(
    db_path: &Path,
    text_content: Option<String>,
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
            "INSERT INTO messages (text_content, archived) VALUES (?, 0)",
            params![text_content],
        )
        .map_err(|err| format!("新增图文消息失败：{err}"))?;

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
