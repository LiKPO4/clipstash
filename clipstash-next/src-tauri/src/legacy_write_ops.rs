use crate::{
    legacy_backup::{backup_message_image_files, create_legacy_db_backup_for_path},
    legacy_data::{
        LegacyArchiveMessageResult, LegacyCreateTextMessageResult, LegacyDeleteMessageResult,
        LegacyReplaceImagesResult, LegacyUpdateMessageResult,
    },
    legacy_write_audit::legacy_write_audit,
    legacy_write_exec::{
        create_image_message_for_path, create_mixed_message_for_path, create_text_message_for_path,
        delete_message_for_path, replace_message_images_for_path, set_message_archived_for_path,
        update_text_message_for_path,
    },
    legacy_write_precheck::{
        ensure_message_exists_for_path, read_message_for_update_precheck,
        validate_replace_images_request,
    },
    legacy_write_validation::{
        normalize_optional_text_message, normalize_text_message, validate_images_data,
    },
};
use std::path::Path;

pub(crate) fn create_text_message_with_backup_for_path(
    db_path: &Path,
    text_content: String,
) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized_text = normalize_text_message(text_content)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_text_message_for_path(db_path, Some(normalized_text))
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("create_text_message", &message, &backup, None);
    Ok(LegacyCreateTextMessageResult {
        backup,
        audit,
        message,
    })
}

pub(crate) fn create_image_message_with_backup_for_path(
    db_path: &Path,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    validate_images_data(&images_data)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_image_message_for_path(db_path, images_data)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("create_image_message", &message, &backup, None);
    Ok(LegacyCreateTextMessageResult {
        backup,
        audit,
        message,
    })
}

pub(crate) fn create_mixed_message_with_backup_for_path(
    db_path: &Path,
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyCreateTextMessageResult, String> {
    let normalized_text = normalize_text_message(text_content)?;
    validate_images_data(&images_data)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = create_mixed_message_for_path(db_path, Some(normalized_text), images_data)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("create_mixed_message", &message, &backup, None);
    Ok(LegacyCreateTextMessageResult {
        backup,
        audit,
        message,
    })
}

pub(crate) fn update_text_message_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    text_content: Option<String>,
) -> Result<LegacyUpdateMessageResult, String> {
    let normalized_text = normalize_optional_text_message(text_content);
    ensure_message_exists_for_path(db_path, message_id)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = update_text_message_for_path(db_path, message_id, normalized_text)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("update_message_text", &message, &backup, None);
    Ok(LegacyUpdateMessageResult {
        backup,
        audit,
        message,
    })
}

pub(crate) fn replace_message_images_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<LegacyReplaceImagesResult, String> {
    validate_replace_images_request(db_path, message_id, &images_data)?;
    let current_message = read_message_for_update_precheck(db_path, message_id)?;
    let data_dir = db_path.parent().ok_or_else(|| {
        format!(
            "替换消息图片失败，无法定位数据库目录：{}",
            db_path.display()
        )
    })?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let image_backup = backup_message_image_files(data_dir, &current_message.images)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;
    let message = replace_message_images_for_path(db_path, message_id, images_data)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit(
        "replace_message_images",
        &message,
        &backup,
        image_backup.as_ref(),
    );
    Ok(LegacyReplaceImagesResult {
        backup,
        audit,
        image_backup,
        message,
    })
}

pub(crate) fn delete_message_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
) -> Result<LegacyDeleteMessageResult, String> {
    let current_message = read_message_for_update_precheck(db_path, message_id)?;
    let data_dir = db_path
        .parent()
        .ok_or_else(|| format!("删除消息失败，无法定位数据库目录：{}", db_path.display()))?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let image_backup = backup_message_image_files(data_dir, &current_message.images)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;
    let message = delete_message_for_path(db_path, message_id)
        .map_err(|err| format!("{err}；已创建数据库备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("delete_message", &message, &backup, image_backup.as_ref());
    Ok(LegacyDeleteMessageResult {
        backup,
        audit,
        image_backup,
        message,
    })
}

pub(crate) fn set_message_archived_with_backup_for_path(
    db_path: &Path,
    message_id: i64,
    archived: bool,
) -> Result<LegacyArchiveMessageResult, String> {
    ensure_message_exists_for_path(db_path, message_id)?;
    let backup = create_legacy_db_backup_for_path(db_path)?;
    let message = set_message_archived_for_path(db_path, message_id, archived)
        .map_err(|err| format!("{err}；已创建备份：{}", backup.backup_path))?;

    let audit = legacy_write_audit("set_message_archived", &message, &backup, None);
    Ok(LegacyArchiveMessageResult {
        backup,
        audit,
        message,
    })
}
