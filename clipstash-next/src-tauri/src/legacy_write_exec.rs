use crate::{
    legacy_image_files::{
        next_image_filename, remove_old_message_image_files, save_image_file, sniff_image_extension,
    },
    legacy_model::{LegacyMessage, MergeDirection, MessageView, SortOrder},
    legacy_query::{read_legacy_message_by_id, view_where_sql},
    legacy_schema::{configure_connection, ensure_legacy_schema},
    legacy_write_precheck::validate_replace_images_request,
    legacy_write_validation::validate_images_data,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
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
    configure_connection(&conn)?;
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
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let mut saved_images = Vec::new();
    for (index, image_data) in images_data.iter().enumerate() {
        let filename = next_image_filename(&images_dir, index, sniff_image_extension(image_data));
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

/// 把选中的文字拆成一条新消息，原消息保留剩余内容（图片仍留在原消息）。
pub(crate) fn split_message_selection_for_path(
    db_path: &Path,
    message_id: i64,
    selected_text: String,
    remaining_text: Option<String>,
) -> Result<(LegacyMessage, LegacyMessage), String> {
    if message_id <= 0 {
        return Err("拆分消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("拆分消息失败，数据库不存在：{}", db_path.display()));
    }

    let selected = selected_text.trim().to_string();
    if selected.is_empty() {
        return Err("拆分消息失败，选中的内容为空".to_string());
    }
    let remaining = remaining_text
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("拆分消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");

    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开数据库准备拆分失败：{err}"))?;
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;
    let old_message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;

    let tx = conn
        .transaction()
        .map_err(|err| format!("开启消息拆分事务失败：{err}"))?;
    tx.execute(
        "INSERT INTO messages (text_content, created_at, archived, archived_at) VALUES (?, ?, ?, ?)",
        params![
            selected,
            old_message.created_at,
            if old_message.archived { 1 } else { 0 },
            old_message.archived_at,
        ],
    )
    .map_err(|err| format!("写入拆出的新消息失败：{err}"))?;
    let new_message_id = tx.last_insert_rowid();
    let updated = tx
        .execute(
            "UPDATE messages SET text_content = ? WHERE id = ?",
            params![remaining, message_id],
        )
        .map_err(|err| format!("更新原消息剩余内容失败：{err}"))?;
    if updated == 0 {
        return Err(format!("拆分消息失败，原消息不存在：{message_id}"));
    }
    tx.commit()
        .map_err(|err| format!("提交消息拆分事务失败：{err}"))?;

    let message = read_legacy_message_by_id(&conn, &images_dir, message_id)?;
    let new_message = read_legacy_message_by_id(&conn, &images_dir, new_message_id)?;
    Ok((message, new_message))
}

pub(crate) fn merge_message_with_neighbor_for_path(
    db_path: &Path,
    message_id: i64,
    direction: MergeDirection,
    view: MessageView,
    sort: SortOrder,
) -> Result<(LegacyMessage, i64), String> {
    if message_id <= 0 {
        return Err("合并消息失败，消息 id 必须大于 0".to_string());
    }
    if !db_path.is_file() {
        return Err(format!("合并消息失败，数据库不存在：{}", db_path.display()));
    }

    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("合并消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let images_dir = data_dir.join("images");
    let mut conn =
        Connection::open(db_path).map_err(|err| format!("打开数据库准备合并失败：{err}"))?;
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;
    let anchor = read_legacy_message_by_id(&conn, &images_dir, message_id)?;
    let neighbor_id = find_neighbor_message_id(&conn, &anchor, direction, view, sort)?;
    let neighbor = read_legacy_message_by_id(&conn, &images_dir, neighbor_id)?;

    // 合并后的内容顺序与显示顺序一致：向上合并时邻居内容在前，向下合并时邻居内容在后。
    let (first, second) = match direction {
        MergeDirection::Up => (&neighbor, &anchor),
        MergeDirection::Down => (&anchor, &neighbor),
    };
    let merged_text = merge_text_contents(
        first.text_content.as_deref(),
        second.text_content.as_deref(),
    );
    let mut merged_filenames = Vec::with_capacity(first.images.len() + second.images.len());
    merged_filenames.extend(first.images.iter().map(|image| image.filename.clone()));
    merged_filenames.extend(second.images.iter().map(|image| image.filename.clone()));

    // 图片文件被保留的消息继续引用，不移动、不删除；只重排关联行保证显示顺序。
    let merge_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启消息合并事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![anchor.id],
        )
        .map_err(|err| format!("重排合并消息图片关联失败：{err}"))?;
        for filename in &merged_filenames {
            tx.execute(
                "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                params![anchor.id, filename],
            )
            .map_err(|err| format!("关联合并消息图片失败：{err}"))?;
        }
        tx.execute(
            "UPDATE messages SET text_content = ? WHERE id = ?",
            params![merged_text, anchor.id],
        )
        .map_err(|err| format!("更新合并消息文字失败：{err}"))?;
        tx.execute(
            "DELETE FROM message_images WHERE message_id = ?",
            params![neighbor.id],
        )
        .map_err(|err| format!("删除被合并消息图片关联失败：{err}"))?;
        let deleted = tx
            .execute("DELETE FROM messages WHERE id = ?", params![neighbor.id])
            .map_err(|err| format!("删除被合并消息失败：{err}"))?;
        if deleted == 0 {
            return Err(format!("合并消息失败，被合并消息不存在：{}", neighbor.id));
        }
        tx.commit()
            .map_err(|err| format!("提交消息合并事务失败：{err}"))
    })();
    merge_result?;

    let merged = read_legacy_message_by_id(&conn, &images_dir, anchor.id)?;
    Ok((merged, neighbor.id))
}

fn merge_text_contents(first: Option<&str>, second: Option<&str>) -> Option<String> {
    let first = first.map(str::trim).filter(|text| !text.is_empty());
    let second = second.map(str::trim).filter(|text| !text.is_empty());
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}\n{second}")),
        (Some(first), None) => Some(first.to_string()),
        (None, Some(second)) => Some(second.to_string()),
        (None, None) => None,
    }
}

/// 在当前视图和排序下找到显示顺序中紧邻的消息 id。
fn find_neighbor_message_id(
    conn: &Connection,
    anchor: &LegacyMessage,
    direction: MergeDirection,
    view: MessageView,
    sort: SortOrder,
) -> Result<i64, String> {
    let where_sql = view_where_sql(view);
    let sort_expr = match view {
        MessageView::Normal => "created_at",
        MessageView::Archived => "COALESCE(archived_at, created_at)",
    };
    let anchor_sort_value = match view {
        MessageView::Normal => anchor.created_at.clone(),
        MessageView::Archived => anchor
            .archived_at
            .clone()
            .unwrap_or_else(|| anchor.created_at.clone()),
    };
    // 显示顺序：newest 降序、oldest 升序；“下方”沿显示顺序前进，“上方”相反。
    let (compare_op, order_dir, label) = match (direction, sort) {
        (MergeDirection::Down, SortOrder::Newest) => ("<", "DESC", "下方"),
        (MergeDirection::Up, SortOrder::Newest) => (">", "ASC", "上方"),
        (MergeDirection::Down, SortOrder::Oldest) => (">", "ASC", "下方"),
        (MergeDirection::Up, SortOrder::Oldest) => ("<", "DESC", "上方"),
    };
    let sql = format!(
        "SELECT id FROM messages \
         WHERE {where_sql} \
           AND ({sort_expr} {compare_op} ?1 OR ({sort_expr} = ?1 AND id {compare_op} ?2)) \
         ORDER BY {sort_expr} {order_dir}, id {order_dir} \
         LIMIT 1"
    );
    conn.query_row(&sql, params![anchor_sort_value, anchor.id], |row| {
        row.get::<_, i64>(0)
    })
    .optional()
    .map_err(|err| format!("查找相邻消息失败：{err}"))?
    .ok_or_else(|| format!("合并消息失败，{label}没有相邻消息"))
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
    configure_connection(&conn)?;
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
            let filename =
                next_image_filename(&images_dir, index, sniff_image_extension(image_data));
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
    configure_connection(&conn)?;
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
    configure_connection(&conn)?;
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
    configure_connection(&conn)?;
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
    create_message_with_image_reader(db_path, text_content, images_data.into_iter().map(Ok))
}

pub(crate) fn create_message_from_image_files_for_path(
    db_path: &Path,
    text_content: Option<String>,
    files: Vec<std::path::PathBuf>,
) -> Result<LegacyMessage, String> {
    if files.is_empty() {
        return create_text_message_for_path(db_path, text_content);
    }
    create_message_with_image_reader(
        db_path,
        text_content,
        files.into_iter().map(|path| {
            use std::io::Read;
            let mut bytes = Vec::new();
            fs::File::open(&path)
                .map_err(|e| e.to_string())?
                .take(256 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.is_empty() || bytes.len() > 256 * 1024 * 1024 {
                return Err("分享图片为空或超过 256MiB".into());
            }
            Ok(bytes)
        }),
    )
}

fn create_message_with_image_reader(
    db_path: &Path,
    text_content: Option<String>,
    images_data: impl Iterator<Item = Result<Vec<u8>, String>>,
) -> Result<LegacyMessage, String> {
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
    configure_connection(&conn)?;
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
        for (index, image_data) in images_data.enumerate() {
            let image_data = image_data?;
            let filename =
                next_image_filename(&images_dir, index, sniff_image_extension(&image_data));
            let path = images_dir.join(&filename);
            saved_paths.push(path.clone());
            save_image_file(&path, &image_data)?;
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

#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::legacy_test_support::tiny_png_bytes;
    use rusqlite::Connection;
    use std::{env, fs, path::PathBuf, process};

    struct MergeFixture {
        db_path: PathBuf,
        images_dir: PathBuf,
    }

    fn create_fixture(name: &str) -> MergeFixture {
        let data_dir =
            env::temp_dir().join(format!("clipstash-next-merge-{name}-{}", process::id()));
        let _ = fs::remove_dir_all(&data_dir);
        fs::create_dir_all(data_dir.join("images")).expect("create merge fixture dir");
        let db_path = data_dir.join("clipstash.db");
        let conn = Connection::open(&db_path).expect("open merge fixture db");
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
        .expect("seed merge schema");
        drop(conn);
        MergeFixture {
            db_path,
            images_dir: data_dir.join("images"),
        }
    }

    fn seed_message(
        fixture: &MergeFixture,
        id: i64,
        text: Option<&str>,
        created_at: &str,
        archived: i64,
        archived_at: Option<&str>,
    ) {
        let conn = Connection::open(&fixture.db_path).expect("reopen fixture db");
        conn.execute(
            "INSERT INTO messages (id, text_content, created_at, archived, archived_at) VALUES (?, ?, ?, ?, ?)",
            params![id, text, created_at, archived, archived_at],
        )
        .expect("seed merge message");
    }

    fn seed_image(fixture: &MergeFixture, message_id: i64, filename: &str) {
        fs::write(fixture.images_dir.join(filename), tiny_png_bytes())
            .expect("write fixture image");
        let conn = Connection::open(&fixture.db_path).expect("reopen fixture db");
        conn.execute(
            "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
            params![message_id, filename],
        )
        .expect("seed fixture image row");
    }

    fn message_count(fixture: &MergeFixture) -> i64 {
        let conn = Connection::open(&fixture.db_path).expect("reopen fixture db");
        conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .expect("count messages")
    }

    #[test]
    fn merges_down_with_next_message_in_display_order() {
        let fixture = create_fixture("down");
        // newest 视图显示顺序：#3、#2、#1；#2 下方相邻是 #1。
        seed_message(&fixture, 1, Some("one"), "2026-07-01 10:00:00", 0, None);
        seed_message(&fixture, 2, Some("two"), "2026-07-02 10:00:00", 0, None);
        seed_message(&fixture, 3, Some("three"), "2026-07-03 10:00:00", 0, None);
        seed_image(&fixture, 2, "two-a.png");
        seed_image(&fixture, 2, "two-b.png");
        seed_image(&fixture, 1, "one-a.png");

        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            2,
            MergeDirection::Down,
            MessageView::Normal,
            SortOrder::Newest,
        )
        .expect("merge down");

        assert_eq!(removed_id, 1);
        assert_eq!(merged.id, 2);
        assert_eq!(merged.created_at, "2026-07-02 10:00:00");
        assert_eq!(merged.text_content.as_deref(), Some("two\none"));
        let filenames = merged
            .images
            .iter()
            .map(|image| image.filename.as_str())
            .collect::<Vec<_>>();
        assert_eq!(filenames, vec!["two-a.png", "two-b.png", "one-a.png"]);
        assert!(merged.images.iter().all(|image| image.exists));
        assert_eq!(message_count(&fixture), 2);
    }

    #[test]
    fn merges_up_with_previous_message_in_display_order() {
        let fixture = create_fixture("up");
        seed_message(&fixture, 1, Some("one"), "2026-07-01 10:00:00", 0, None);
        seed_message(&fixture, 2, Some("two"), "2026-07-02 10:00:00", 0, None);

        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            1,
            MergeDirection::Up,
            MessageView::Normal,
            SortOrder::Newest,
        )
        .expect("merge up");

        // newest 视图显示顺序：#2、#1；#1 上方相邻是 #2，邻居内容在前。
        assert_eq!(removed_id, 2);
        assert_eq!(merged.id, 1);
        assert_eq!(merged.text_content.as_deref(), Some("two\none"));
        assert_eq!(message_count(&fixture), 1);
    }

    #[test]
    fn honors_oldest_sort_when_finding_neighbor() {
        let fixture = create_fixture("oldest");
        seed_message(&fixture, 1, Some("one"), "2026-07-01 10:00:00", 0, None);
        seed_message(&fixture, 2, Some("two"), "2026-07-02 10:00:00", 0, None);

        // oldest 视图显示顺序：#1、#2；#2 上方相邻是 #1。
        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            2,
            MergeDirection::Up,
            MessageView::Normal,
            SortOrder::Oldest,
        )
        .expect("merge up in oldest view");

        assert_eq!(removed_id, 1);
        assert_eq!(merged.id, 2);
        assert_eq!(merged.text_content.as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn rejects_merge_without_neighbor() {
        let fixture = create_fixture("edge");
        seed_message(&fixture, 1, Some("one"), "2026-07-01 10:00:00", 0, None);

        let error = match merge_message_with_neighbor_for_path(
            &fixture.db_path,
            1,
            MergeDirection::Down,
            MessageView::Normal,
            SortOrder::Newest,
        ) {
            Err(err) => err,
            Ok(_) => panic!("merge without neighbor should fail"),
        };

        assert!(
            error.contains("下方没有相邻消息"),
            "unexpected error: {error}"
        );
        assert_eq!(message_count(&fixture), 1);
    }

    #[test]
    fn merge_skips_messages_outside_current_view() {
        let fixture = create_fixture("view");
        seed_message(&fixture, 1, Some("one"), "2026-07-01 10:00:00", 0, None);
        seed_message(&fixture, 2, Some("two"), "2026-07-02 10:00:00", 0, None);
        seed_message(
            &fixture,
            3,
            Some("archived"),
            "2026-07-03 10:00:00",
            1,
            None,
        );

        // newest 普通视图显示顺序：#2、#1；#3 已归档，不能成为普通视图的合并邻居。
        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            2,
            MergeDirection::Down,
            MessageView::Normal,
            SortOrder::Newest,
        )
        .expect("merge down in normal view");

        assert_eq!(removed_id, 1);
        assert_eq!(merged.id, 2);
        assert_eq!(merged.text_content.as_deref(), Some("two\none"));
        assert_eq!(message_count(&fixture), 2);
    }

    #[test]
    fn archived_view_orders_neighbors_by_archived_at() {
        let fixture = create_fixture("archived");
        seed_message(
            &fixture,
            1,
            Some("one"),
            "2026-07-01 10:00:00",
            1,
            Some("2026-07-05 10:00:00"),
        );
        seed_message(
            &fixture,
            2,
            Some("two"),
            "2026-07-02 10:00:00",
            1,
            Some("2026-07-04 10:00:00"),
        );

        // 归档视图按 COALESCE(archived_at, created_at) 降序：#1 在上、#2 在下。
        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            2,
            MergeDirection::Up,
            MessageView::Archived,
            SortOrder::Newest,
        )
        .expect("merge up in archived view");

        assert_eq!(removed_id, 1);
        assert_eq!(merged.id, 2);
        assert!(merged.archived);
        assert_eq!(merged.text_content.as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn merge_keeps_text_and_images_when_one_side_is_empty() {
        let fixture = create_fixture("empty");
        seed_message(&fixture, 1, None, "2026-07-01 10:00:00", 0, None);
        seed_image(&fixture, 1, "one-a.png");
        seed_message(&fixture, 2, Some("two"), "2026-07-02 10:00:00", 0, None);

        let (merged, removed_id) = merge_message_with_neighbor_for_path(
            &fixture.db_path,
            2,
            MergeDirection::Down,
            MessageView::Normal,
            SortOrder::Newest,
        )
        .expect("merge down with empty neighbor text");

        assert_eq!(removed_id, 1);
        assert_eq!(merged.text_content.as_deref(), Some("two"));
        let filenames = merged
            .images
            .iter()
            .map(|image| image.filename.as_str())
            .collect::<Vec<_>>();
        assert_eq!(filenames, vec!["one-a.png"]);
    }
}
