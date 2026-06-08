mod import_executor;
mod keyboard_input;
mod legacy_data;
mod window_targets;

#[tauri::command]
fn get_legacy_stats() -> Result<legacy_data::LegacyStats, String> {
    legacy_data::read_legacy_stats()
}

#[tauri::command]
fn list_external_window_targets() -> Result<Vec<window_targets::ExternalWindowTarget>, String> {
    window_targets::list_external_window_targets()
}

#[tauri::command]
fn validate_external_window_target(
    hwnd: isize,
) -> Result<window_targets::ExternalWindowValidation, String> {
    window_targets::validate_external_window_target(hwnd)
}

#[tauri::command]
fn create_legacy_db_backup() -> Result<legacy_data::LegacyDbBackup, String> {
    legacy_data::create_legacy_db_backup()
}

#[tauri::command]
fn create_legacy_text_message(
    text_content: String,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    legacy_data::create_legacy_text_message(text_content)
}

#[tauri::command]
fn create_legacy_image_message(
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    legacy_data::create_legacy_image_message(images_data)
}

#[tauri::command]
fn create_legacy_mixed_message(
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    legacy_data::create_legacy_mixed_message(text_content, images_data)
}

#[tauri::command]
fn update_legacy_message_text(
    message_id: i64,
    text_content: Option<String>,
) -> Result<legacy_data::LegacyUpdateMessageResult, String> {
    legacy_data::update_legacy_message_text(message_id, text_content)
}

#[tauri::command]
fn replace_legacy_message_images(
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyReplaceImagesResult, String> {
    legacy_data::replace_legacy_message_images(message_id, images_data)
}

#[tauri::command]
fn delete_legacy_message(
    message_id: i64,
) -> Result<legacy_data::LegacyDeleteMessageResult, String> {
    legacy_data::delete_legacy_message(message_id)
}

#[tauri::command]
fn set_legacy_message_archived(
    message_id: i64,
    archived: bool,
) -> Result<legacy_data::LegacyArchiveMessageResult, String> {
    legacy_data::set_legacy_message_archived(message_id, archived)
}

#[tauri::command]
fn copy_legacy_image_to_clipboard(
    filename: String,
) -> Result<legacy_data::LegacyCopyImageResult, String> {
    legacy_data::copy_legacy_image_to_clipboard(filename)
}

#[tauri::command]
fn copy_legacy_message_import_queue_item_to_clipboard(
    message_id: i64,
    item_index: usize,
) -> Result<legacy_data::LegacyImportQueueCopyResult, String> {
    legacy_data::copy_legacy_message_import_queue_item_to_clipboard(message_id, item_index)
}

#[tauri::command]
fn paste_legacy_import_queue_item(
    message_id: i64,
    item_index: usize,
    target_hwnd: isize,
) -> Result<import_executor::LegacyImportPasteResult, String> {
    import_executor::paste_legacy_import_queue_item(message_id, item_index, target_hwnd)
}

#[tauri::command]
fn paste_legacy_import_queue(
    message_id: i64,
    target_hwnd: isize,
    delay_ms: Option<u64>,
) -> Result<import_executor::LegacyImportQueuePasteResult, String> {
    import_executor::paste_legacy_import_queue(message_id, target_hwnd, delay_ms)
}

#[tauri::command]
fn stage_legacy_message_import_to_clipboard(
    message_id: i64,
) -> Result<legacy_data::LegacyImportStageResult, String> {
    legacy_data::stage_legacy_message_import_to_clipboard(message_id)
}

#[tauri::command]
fn preview_legacy_message_import_queue(
    message_id: i64,
) -> Result<legacy_data::LegacyImportQueuePreview, String> {
    legacy_data::preview_legacy_message_import_queue(message_id)
}

#[tauri::command]
fn list_legacy_messages(
    view: legacy_data::MessageView,
    sort: legacy_data::SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<legacy_data::LegacyMessagePage, String> {
    legacy_data::list_legacy_messages(view, sort, offset, limit)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            create_legacy_db_backup,
            create_legacy_image_message,
            create_legacy_mixed_message,
            create_legacy_text_message,
            copy_legacy_image_to_clipboard,
            copy_legacy_message_import_queue_item_to_clipboard,
            delete_legacy_message,
            get_legacy_stats,
            list_external_window_targets,
            list_legacy_messages,
            paste_legacy_import_queue,
            paste_legacy_import_queue_item,
            preview_legacy_message_import_queue,
            replace_legacy_message_images,
            set_legacy_message_archived,
            stage_legacy_message_import_to_clipboard,
            update_legacy_message_text,
            validate_external_window_target
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
