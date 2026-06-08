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
            get_legacy_stats,
            list_legacy_messages
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
