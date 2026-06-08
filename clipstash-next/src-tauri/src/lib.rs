mod legacy_data;

#[tauri::command]
fn get_legacy_stats() -> Result<legacy_data::LegacyStats, String> {
    legacy_data::read_legacy_stats()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_legacy_stats])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
