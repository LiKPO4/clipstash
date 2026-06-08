pub(crate) fn normalize_text_message(text_content: String) -> Result<String, String> {
    let normalized = text_content.trim().to_string();
    if normalized.is_empty() {
        return Err("新增纯文字消息失败，文字内容不能为空".to_string());
    }

    Ok(normalized)
}

pub(crate) fn normalize_optional_text_message(text_content: Option<String>) -> Option<String> {
    text_content
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub(crate) fn validate_images_data(images_data: &[Vec<u8>]) -> Result<(), String> {
    if images_data.is_empty() {
        return Err("新增图片消息失败，至少需要一张图片".to_string());
    }
    if images_data.iter().any(|image_data| image_data.is_empty()) {
        return Err("新增图片消息失败，图片数据不能为空".to_string());
    }

    Ok(())
}
