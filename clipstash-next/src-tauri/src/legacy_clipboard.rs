use crate::{
    legacy_image_files::resolve_legacy_image_path,
    legacy_model::{LegacyMessage, LegacyMessageImage},
    legacy_paths::path_to_string,
    legacy_write_precheck::read_message_for_update_precheck,
};
use arboard::{Clipboard, ImageData};
use serde::Serialize;
use std::{borrow::Cow, path::PathBuf};

#[derive(Debug, Serialize)]
pub struct LegacyCopyImageResult {
    pub filename: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize)]
pub struct LegacyCopyTextResult {
    pub message_id: i64,
    pub text_length: usize,
}

#[derive(Serialize)]
pub struct LegacyImportStageResult {
    pub message_id: i64,
    pub staged_kind: String,
    pub text_length: usize,
    pub image_count: usize,
    pub first_image_filename: Option<String>,
    pub copied_image: Option<LegacyCopyImageResult>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueueItem {
    pub kind: String,
    pub text: Option<String>,
    pub text_length: usize,
    pub image: Option<LegacyMessageImage>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueuePreview {
    pub message_id: i64,
    pub item_count: usize,
    pub text_length: usize,
    pub image_count: usize,
    pub skipped_missing_image_count: usize,
    pub items: Vec<LegacyImportQueueItem>,
}

#[derive(Debug, Serialize)]
pub struct LegacyImportQueueCopyResult {
    pub message_id: i64,
    pub item_index: usize,
    pub staged_kind: String,
    pub text_length: usize,
    pub image_filename: Option<String>,
    pub copied_image: Option<LegacyCopyImageResult>,
}

pub(crate) fn copy_legacy_image_to_clipboard_from_dir(
    data_dir: PathBuf,
    filename: String,
) -> Result<LegacyCopyImageResult, String> {
    let image_path = resolve_legacy_image_path(&data_dir, &filename)?;
    let image = image::open(&image_path)
        .map_err(|err| format!("读取旧图片准备复制失败：{}：{err}", image_path.display()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let bytes = image.into_raw();

    let mut clipboard =
        Clipboard::new().map_err(|err| format!("打开系统剪贴板准备复制图片失败：{err}"))?;
    clipboard
        .set_image(ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(bytes),
        })
        .map_err(|err| format!("写入图片到系统剪贴板失败：{err}"))?;

    Ok(LegacyCopyImageResult {
        filename,
        path: path_to_string(image_path),
        width,
        height,
    })
}

pub(crate) fn copy_legacy_message_text_to_clipboard_from_dir(
    data_dir: PathBuf,
    message_id: i64,
) -> Result<LegacyCopyTextResult, String> {
    let db_path = data_dir.join("clipstash.db");
    let message = read_message_for_update_precheck(&db_path, message_id)?;
    let text = message
        .text_content
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("消息 #{message_id} 没有可复制的文字"))?;

    let mut clipboard =
        Clipboard::new().map_err(|err| format!("打开系统剪贴板准备复制文字失败：{err}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| format!("写入文字到系统剪贴板失败：{err}"))?;

    Ok(LegacyCopyTextResult {
        message_id,
        text_length: text.chars().count(),
    })
}

pub(crate) fn stage_legacy_message_import_to_clipboard_from_dir(
    data_dir: PathBuf,
    message_id: i64,
) -> Result<LegacyImportStageResult, String> {
    let db_path = data_dir.join("clipstash.db");
    let message = read_message_for_update_precheck(&db_path, message_id)?;
    let text = message
        .text_content
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let first_existing_image = message.images.iter().find(|image| image.exists);
    let text_length = text.map(|value| value.chars().count()).unwrap_or(0);
    let image_count = message.images.iter().filter(|image| image.exists).count();
    let first_image_filename = first_existing_image.map(|image| image.filename.clone());

    if let Some(text) = text {
        let mut clipboard =
            Clipboard::new().map_err(|err| format!("打开系统剪贴板准备导入文字失败：{err}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| format!("写入文字到系统剪贴板失败：{err}"))?;

        return Ok(LegacyImportStageResult {
            message_id,
            staged_kind: "text".to_string(),
            text_length,
            image_count,
            first_image_filename,
            copied_image: None,
        });
    }

    if let Some(image) = first_existing_image {
        let copied_image =
            copy_legacy_image_to_clipboard_from_dir(data_dir, image.filename.clone())?;
        return Ok(LegacyImportStageResult {
            message_id,
            staged_kind: "image".to_string(),
            text_length,
            image_count,
            first_image_filename,
            copied_image: Some(copied_image),
        });
    }

    Err(format!(
        "导入消息失败，消息为空或图片文件缺失：#{message_id}"
    ))
}

pub(crate) fn preview_legacy_message_import_queue_from_dir(
    data_dir: PathBuf,
    message_id: i64,
) -> Result<LegacyImportQueuePreview, String> {
    let db_path = data_dir.join("clipstash.db");
    let message = read_message_for_update_precheck(&db_path, message_id)?;
    import_queue_preview_from_message(message)
}

fn import_queue_preview_from_message(
    message: LegacyMessage,
) -> Result<LegacyImportQueuePreview, String> {
    let text = message
        .text_content
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let text_length = text.map(|value| value.chars().count()).unwrap_or(0);
    let existing_images: Vec<LegacyMessageImage> = message
        .images
        .iter()
        .filter(|image| image.exists)
        .cloned()
        .collect();
    let skipped_missing_image_count = message.images.len().saturating_sub(existing_images.len());

    let mut items = Vec::new();
    if let Some(text) = text {
        items.push(LegacyImportQueueItem {
            kind: "text".to_string(),
            text: Some(text.to_string()),
            text_length,
            image: None,
        });
    }
    for image in existing_images.iter().cloned() {
        items.push(LegacyImportQueueItem {
            kind: "image".to_string(),
            text: None,
            text_length: 0,
            image: Some(image),
        });
    }

    if items.is_empty() {
        return Err(format!(
            "导入消息失败，消息为空或图片文件缺失：#{}",
            message.id
        ));
    }

    Ok(LegacyImportQueuePreview {
        message_id: message.id,
        item_count: items.len(),
        text_length,
        image_count: existing_images.len(),
        skipped_missing_image_count,
        items,
    })
}

pub(crate) fn copy_legacy_message_import_queue_item_to_clipboard_from_dir(
    data_dir: PathBuf,
    message_id: i64,
    item_index: usize,
) -> Result<LegacyImportQueueCopyResult, String> {
    let preview = preview_legacy_message_import_queue_from_dir(data_dir.clone(), message_id)?;
    let item = preview.items.get(item_index).ok_or_else(|| {
        format!(
            "复制导入队列项失败，索引超出范围：#{message_id} index={item_index} total={}",
            preview.item_count
        )
    })?;

    if item.kind == "text" {
        let text = item
            .text
            .as_deref()
            .ok_or_else(|| format!("复制导入队列文字失败，队列项缺少文字：#{message_id}"))?;
        let mut clipboard =
            Clipboard::new().map_err(|err| format!("打开系统剪贴板准备导入文字失败：{err}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| format!("写入文字到系统剪贴板失败：{err}"))?;

        return Ok(LegacyImportQueueCopyResult {
            message_id,
            item_index,
            staged_kind: "text".to_string(),
            text_length: item.text_length,
            image_filename: None,
            copied_image: None,
        });
    }

    if item.kind == "image" {
        let image = item
            .image
            .as_ref()
            .ok_or_else(|| format!("复制导入队列图片失败，队列项缺少图片：#{message_id}"))?;
        let copied_image =
            copy_legacy_image_to_clipboard_from_dir(data_dir, image.filename.clone())?;

        return Ok(LegacyImportQueueCopyResult {
            message_id,
            item_index,
            staged_kind: "image".to_string(),
            text_length: 0,
            image_filename: Some(image.filename.clone()),
            copied_image: Some(copied_image),
        });
    }

    Err(format!("复制导入队列项失败，未知队列项类型：{}", item.kind))
}
