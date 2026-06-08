mod legacy_data;

#[tauri::command]
fn get_legacy_stats() -> Result<legacy_data::LegacyStats, String> {
    legacy_data::read_legacy_stats()
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
            get_legacy_stats,
            list_legacy_messages,
            replace_legacy_message_images,
            update_legacy_message_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
