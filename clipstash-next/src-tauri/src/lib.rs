mod app_data;
mod app_settings;
mod data_transfer;
mod import_executor;
mod keyboard_input;
mod legacy_backup;
mod legacy_clipboard;
mod legacy_data;
mod legacy_image_files;
mod legacy_model;
mod legacy_paths;
mod legacy_query;
#[cfg(test)]
mod legacy_read_tests;
mod legacy_safety;
mod legacy_schema;
#[cfg(test)]
mod legacy_test_support;
mod legacy_write_audit;
mod legacy_write_exec;
mod legacy_write_ops;
mod legacy_write_precheck;
mod legacy_write_validation;
mod window_targets;

use std::fs;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Manager, WindowEvent};

#[cfg(target_os = "windows")]
use arboard::Clipboard;
#[cfg(target_os = "windows")]
use image::{ImageBuffer, ImageFormat, Rgba};
#[cfg(target_os = "windows")]
use tauri::menu::{Menu, MenuItem};
#[cfg(target_os = "windows")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(target_os = "windows")]
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
#[cfg(target_os = "windows")]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
const TRAY_SHOW_HIDE_ID: &str = "tray_show_hide";
const TRAY_OPEN_DATA_DIR_ID: &str = "tray_open_data_dir";
const TRAY_QUIT_ID: &str = "tray_quit";
#[derive(Debug, serde::Serialize)]
struct CaptureClipboardResult {
    kind: String,
    message_id: i64,
    text_length: usize,
    image_count: usize,
}

#[derive(Debug, serde::Serialize)]
struct ClipboardContent {
    kind: String,
    text: Option<String>,
    image_data: Option<Vec<u8>>,
}

#[derive(Debug, serde::Serialize)]
struct DownloadUpdateResult {
    installer_path: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct GithubReleaseAsset {
    browser_download_url: Option<String>,
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct GithubReleaseInfo {
    assets: Option<Vec<GithubReleaseAsset>>,
    html_url: Option<String>,
    tag_name: Option<String>,
}

#[derive(Default)]
struct GlobalShortcutStatus {
    errors: Mutex<Vec<String>>,
    #[cfg(target_os = "windows")]
    show_shortcut: Mutex<Option<Shortcut>>,
    #[cfg(target_os = "windows")]
    capture_shortcut: Mutex<Option<Shortcut>>,
}

#[tauri::command]
fn get_legacy_stats() -> Result<legacy_data::LegacyStats, String> {
    let stats = app_data::read_app_stats()?;
    Ok(legacy_data::LegacyStats {
        data_dir: stats.data_dir,
        db_path: stats.db_path,
        images_dir: stats.images_dir,
        db_exists: stats.db_exists,
        images_dir_exists: stats.images_dir_exists,
        normal_count: stats.normal_count,
        archived_count: stats.archived_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn migrate_legacy_data() -> Result<app_data::AppMigrationResult, String> {
    app_data::migrate_legacy_data()
}

#[tauri::command]
fn export_normal_data_zip() -> Result<data_transfer::DataExportResult, String> {
    let Some(output_path) = pick_save_zip_file_with_windows_dialog()? else {
        return Err("已取消导出数据".to_string());
    };
    data_transfer::export_normal_data_zip_to_path(output_path)
}

#[tauri::command]
fn export_normal_data_zip_bytes() -> Result<data_transfer::DataExportBytesResult, String> {
    data_transfer::export_normal_data_zip_to_temp_bytes()
}

#[tauri::command]
fn import_data_zip() -> Result<data_transfer::DataImportResult, String> {
    let Some(zip_path) = pick_open_zip_file_with_windows_dialog()? else {
        return Err("已取消导入数据".to_string());
    };
    data_transfer::import_data_zip_from_path(zip_path)
}

#[tauri::command]
fn preview_data_zip() -> Result<data_transfer::DataImportPreview, String> {
    let Some(zip_path) = pick_open_zip_file_with_windows_dialog()? else {
        return Err("已取消导入数据".to_string());
    };
    data_transfer::preview_data_zip_from_path(zip_path)
}

#[tauri::command]
fn import_data_zip_from_path(path: String) -> Result<data_transfer::DataImportResult, String> {
    data_transfer::import_data_zip_from_path(std::path::PathBuf::from(path.trim()))
}

#[tauri::command]
fn import_data_zip_bytes(
    filename: String,
    bytes: Vec<u8>,
) -> Result<data_transfer::DataImportResult, String> {
    data_transfer::import_data_zip_from_bytes(filename, bytes)
}

#[tauri::command]
fn open_app_path(path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(path.trim());
    if !path.is_dir() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    open_path_in_file_manager(&path)
}

#[tauri::command]
fn move_app_data_to_selected_dir() -> Result<app_data::AppDataMoveResult, String> {
    let Some(target_dir) = pick_folder_with_windows_dialog()? else {
        return Err("已取消选择数据目录".to_string());
    };
    app_data::move_app_data_to_dir(target_dir)
}

#[tauri::command]
fn repair_app_data_dir() -> Result<app_data::AppDataRepairResult, String> {
    app_data::repair_app_data_dir()
}

#[tauri::command]
fn fetch_latest_github_release() -> Result<GithubReleaseInfo, String> {
    let response = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/LiKPO4/clipstash/releases/latest")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, "ClipStash-Next-Update-Checker")
        .send()
        .map_err(|err| format!("GitHub Release 检查失败：{err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub Release 检查失败：HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .map_err(|err| format!("读取 GitHub Release 响应失败：{err}"))?;
    serde_json::from_str::<GithubReleaseInfo>(&body)
        .map_err(|err| format!("解析 GitHub Release 响应失败：{err}"))
}

#[tauri::command]
fn download_and_open_update_installer(
    download_url: String,
    filename: String,
) -> Result<DownloadUpdateResult, String> {
    if !download_url.starts_with("https://github.com/LiKPO4/clipstash/releases/download/") {
        return Err("更新下载链接不是 ClipStash 官方 Release 地址".to_string());
    }

    let safe_filename = sanitize_installer_filename(&filename)?;
    let update_dir = std::env::temp_dir().join("ClipStash Next Updates");
    fs::create_dir_all(&update_dir).map_err(|err| format!("创建更新临时目录失败：{err}"))?;
    let installer_path = update_dir.join(&safe_filename);

    let response = reqwest::blocking::Client::new()
        .get(&download_url)
        .header(reqwest::header::USER_AGENT, "ClipStash-Next-Updater")
        .send()
        .map_err(|err| format!("下载安装包失败：{err}"))?;
    if !response.status().is_success() {
        return Err(format!("下载安装包失败：HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .map_err(|err| format!("读取安装包内容失败：{err}"))?;
    if bytes.is_empty() {
        return Err("下载安装包失败：文件为空".to_string());
    }

    fs::write(&installer_path, &bytes)
        .map_err(|err| format!("写入安装包失败：{}：{err}", installer_path.display()))?;
    Command::new(&installer_path)
        .spawn()
        .map_err(|err| format!("启动安装包失败：{}：{err}", installer_path.display()))?;

    Ok(DownloadUpdateResult {
        installer_path: installer_path.display().to_string(),
    })
}

fn sanitize_installer_filename(filename: &str) -> Result<String, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return Err("安装包文件名为空".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.ends_with(".exe") || lower.ends_with(".msi")) {
        return Err("更新资产不是 Windows 安装包".to_string());
    }
    if trimmed
        .chars()
        .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
        return Err("安装包文件名包含非法字符".to_string());
    }
    Ok(trimmed.to_string())
}

#[tauri::command]
fn capture_current_clipboard() -> Result<CaptureClipboardResult, String> {
    capture_current_clipboard_to_app_data()
}

#[tauri::command]
fn read_current_clipboard() -> Result<ClipboardContent, String> {
    read_current_clipboard_content()
}

#[tauri::command]
fn get_global_shortcut_errors(
    status: tauri::State<'_, GlobalShortcutStatus>,
) -> Result<Vec<String>, String> {
    Ok(status.errors.lock().unwrap().clone())
}

#[tauri::command]
fn get_launch_on_startup(app: AppHandle) -> Result<bool, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        return Ok(false);
    }

    #[cfg(target_os = "windows")]
    {
        app.autolaunch()
            .is_enabled()
            .map_err(|err| format!("读取开机自启动状态失败：{err}"))
    }
}

#[tauri::command]
fn set_launch_on_startup(app: AppHandle, enabled: bool) -> Result<bool, String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        let _ = enabled;
        return Err("开机自启动仅支持 Windows".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let autostart = app.autolaunch();
        if enabled {
            autostart
                .enable()
                .map_err(|err| format!("启用开机自启动失败：{err}"))?;
        } else {
            autostart
                .disable()
                .map_err(|err| format!("关闭开机自启动失败：{err}"))?;
        }

        autostart
            .is_enabled()
            .map_err(|err| format!("读取开机自启动状态失败：{err}"))
    }
}

#[tauri::command]
fn get_app_settings() -> Result<app_settings::AppSettings, String> {
    app_settings::read_settings()
}

#[tauri::command]
fn update_app_settings(
    app: AppHandle,
    patch: app_settings::AppSettingsPatch,
) -> Result<app_settings::AppSettings, String> {
    let settings = app_settings::update_settings(patch)?;
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
    #[cfg(target_os = "windows")]
    reload_global_shortcuts(&app, &settings);
    Ok(settings)
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
fn create_legacy_text_message(
    text_content: String,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    app_data::create_text_message(text_content)
}

#[tauri::command]
fn create_legacy_image_message(
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    app_data::create_image_message(images_data)
}

#[tauri::command]
fn create_legacy_mixed_message(
    text_content: String,
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyCreateTextMessageResult, String> {
    app_data::create_mixed_message(text_content, images_data)
}

#[tauri::command]
fn update_legacy_message_text(
    message_id: i64,
    text_content: Option<String>,
) -> Result<legacy_data::LegacyUpdateMessageResult, String> {
    app_data::update_message_text(message_id, text_content)
}

#[tauri::command]
fn replace_legacy_message_images(
    message_id: i64,
    images_data: Vec<Vec<u8>>,
) -> Result<legacy_data::LegacyReplaceImagesResult, String> {
    app_data::replace_message_images(message_id, images_data)
}

#[tauri::command]
fn delete_legacy_message(
    message_id: i64,
) -> Result<legacy_data::LegacyDeleteMessageResult, String> {
    app_data::delete_message(message_id)
}

#[tauri::command]
fn set_legacy_message_archived(
    message_id: i64,
    archived: bool,
) -> Result<legacy_data::LegacyArchiveMessageResult, String> {
    app_data::set_message_archived(message_id, archived)
}

#[tauri::command]
fn copy_legacy_image_to_clipboard(
    filename: String,
) -> Result<legacy_data::LegacyCopyImageResult, String> {
    app_data::copy_image_to_clipboard(filename)
}

#[tauri::command]
fn read_legacy_image_bytes(filename: String) -> Result<Vec<u8>, String> {
    app_data::read_image_bytes(filename)
}

#[tauri::command]
fn read_dropped_file_bytes(path: String) -> Result<Vec<u8>, String> {
    let path = std::path::PathBuf::from(path.trim());
    if !path.is_file() {
        return Err(format!("拖入文件不存在：{}", path.display()));
    }
    fs::read(&path).map_err(|err| format!("读取拖入文件失败：{}：{err}", path.display()))
}

#[tauri::command]
fn copy_legacy_message_text_to_clipboard(
    message_id: i64,
) -> Result<legacy_data::LegacyCopyTextResult, String> {
    app_data::copy_message_text_to_clipboard(message_id)
}

#[tauri::command]
fn copy_legacy_message_import_queue_item_to_clipboard(
    message_id: i64,
    item_index: usize,
) -> Result<legacy_data::LegacyImportQueueCopyResult, String> {
    app_data::copy_message_import_queue_item_to_clipboard(message_id, item_index)
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
fn paste_legacy_import_queue_with_optional_archive(
    message_id: i64,
    target_hwnd: isize,
    delay_ms: Option<u64>,
    archive_after_success: bool,
) -> Result<import_executor::LegacyImportQueuePasteArchiveResult, String> {
    import_executor::paste_legacy_import_queue_with_optional_archive(
        message_id,
        target_hwnd,
        delay_ms,
        archive_after_success,
    )
}

#[tauri::command]
fn paste_legacy_import_queue_to_recent_window(
    window: tauri::Window,
    message_id: i64,
    delay_ms: Option<u64>,
    archive_after_success: bool,
) -> Result<import_executor::LegacyImportQueuePasteArchiveResult, String> {
    let target = window_targets::last_external_window_target()
        .ok_or_else(|| "未找到外部输入窗口，已取消导入".to_string())?;
    let was_visible = window.is_visible().unwrap_or(false);

    if was_visible {
        let _ = window.hide();
        std::thread::sleep(Duration::from_millis(120));
    }

    let result = import_executor::paste_legacy_import_queue_with_optional_archive(
        message_id,
        target.hwnd,
        delay_ms,
        archive_after_success,
    );

    if was_visible {
        std::thread::sleep(Duration::from_millis(300));
        let _ = window.show();
        let _ = window.set_focus();
    }

    result
}

#[tauri::command]
fn stage_legacy_message_import_to_clipboard(
    message_id: i64,
) -> Result<legacy_data::LegacyImportStageResult, String> {
    app_data::stage_message_import_to_clipboard(message_id)
}

#[tauri::command]
fn preview_legacy_message_import_queue(
    message_id: i64,
) -> Result<legacy_data::LegacyImportQueuePreview, String> {
    app_data::preview_message_import_queue(message_id)
}

#[tauri::command]
fn list_legacy_messages(
    view: legacy_data::MessageView,
    sort: legacy_data::SortOrder,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<legacy_data::LegacyMessagePage, String> {
    app_data::list_messages(view, sort, offset, limit)
}

#[cfg(target_os = "windows")]
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "windows")]
fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

fn capture_current_clipboard_to_app_data() -> Result<CaptureClipboardResult, String> {
    let content = read_current_clipboard_content()?;

    if let Some(image_data) = content.image_data {
        let result = app_data::create_image_message(vec![image_data])?;

        return Ok(CaptureClipboardResult {
            kind: "image".to_string(),
            message_id: result.message.id,
            text_length: 0,
            image_count: 1,
        });
    }

    if let Some(text) = content.text {
        let text_length = text.chars().count();
        let result = app_data::create_text_message(text)?;

        return Ok(CaptureClipboardResult {
            kind: "text".to_string(),
            message_id: result.message.id,
            text_length,
            image_count: 0,
        });
    }

    Err("剪切板没有可导入内容".to_string())
}

fn read_current_clipboard_content() -> Result<ClipboardContent, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err("读取系统剪贴板仅支持 Windows".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let mut clipboard =
            Clipboard::new().map_err(|err| format!("打开系统剪贴板准备读取失败：{err}"))?;

        if let Ok(image) = clipboard.get_image() {
            return Ok(ClipboardContent {
                kind: "image".to_string(),
                text: None,
                image_data: Some(clipboard_image_to_png(image)?),
            });
        }

        if let Ok(text) = clipboard.get_text() {
            let normalized = text.trim().to_string();
            if !normalized.is_empty() {
                return Ok(ClipboardContent {
                    kind: "text".to_string(),
                    text: Some(normalized),
                    image_data: None,
                });
            }
        }

        Err("剪切板没有可导入内容".to_string())
    }
}

#[cfg(target_os = "windows")]
fn clipboard_image_to_png(image: arboard::ImageData<'_>) -> Result<Vec<u8>, String> {
    let width = image.width as u32;
    let height = image.height as u32;
    let rgba = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, image.bytes.into_owned())
        .ok_or_else(|| "读取剪贴板图片失败：图片像素数据尺寸不匹配".to_string())?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|err| format!("编码剪贴板图片失败：{err}"))?;
    Ok(cursor.into_inner())
}

#[cfg(target_os = "windows")]
fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }

    let shortcut_text = shortcut.into_string();
    if let Some(status) = app.try_state::<GlobalShortcutStatus>() {
        if status
            .show_shortcut
            .lock()
            .unwrap()
            .as_ref()
            .map(|registered| registered.clone().into_string() == shortcut_text)
            .unwrap_or(false)
        {
            toggle_main_window(app);
            return;
        }
        if status
            .capture_shortcut
            .lock()
            .unwrap()
            .as_ref()
            .map(|registered| registered.clone().into_string() == shortcut_text)
            .unwrap_or(false)
        {
            let _ = capture_current_clipboard_to_app_data();
            show_main_window(app);
        }
    }
}

#[cfg(target_os = "windows")]
fn setup_global_shortcuts(app: &tauri::App) {
    let settings = app_settings::read_settings().unwrap_or_default();
    reload_global_shortcuts(app.handle(), &settings);
}

#[cfg(target_os = "windows")]
fn reload_global_shortcuts(app: &AppHandle, settings: &app_settings::AppSettings) {
    let shortcuts = app.global_shortcut();
    let mut errors = Vec::new();

    if let Err(err) = shortcuts.unregister_all() {
        errors.push(format!("清除旧快捷键失败：{err}"));
    }

    let show_shortcut = parse_global_shortcut("呼出界面快捷键", &settings.show_hotkey, &mut errors);
    let capture_shortcut = parse_global_shortcut(
        "导入当前剪切板快捷键",
        &settings.capture_hotkey,
        &mut errors,
    );
    let requested = [&show_shortcut, &capture_shortcut]
        .into_iter()
        .filter_map(|shortcut| shortcut.clone())
        .collect::<Vec<_>>();

    if !requested.is_empty() {
        if let Err(err) = shortcuts.on_shortcuts(requested, handle_global_shortcut) {
            errors.push(format!("快捷键注册失败：{err}"));
        }
    }

    if let Some(status) = app.try_state::<GlobalShortcutStatus>() {
        *status.show_shortcut.lock().unwrap() = show_shortcut;
        *status.capture_shortcut.lock().unwrap() = capture_shortcut;
        *status.errors.lock().unwrap() = errors;
    }
}

#[cfg(target_os = "windows")]
fn parse_global_shortcut(label: &str, value: &str, errors: &mut Vec<String>) -> Option<Shortcut> {
    match value.parse::<Shortcut>() {
        Ok(shortcut) => Some(shortcut),
        Err(err) => {
            errors.push(format!("{label}无效：{err}"));
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn open_app_data_dir() {
    if let Ok(stats) = app_data::read_app_stats() {
        let _ = open_path_in_file_manager(&std::path::PathBuf::from(stats.data_dir));
    }
}

fn open_path_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|err| format!("打开目录失败：{}：{err}", path.display()))?;
        return Ok(());
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("打开本地目录仅支持 Windows".to_string())
    }
}

fn pick_folder_with_windows_dialog() -> Result<Option<std::path::PathBuf>, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = '选择 ClipStash Next 数据目录'
$dialog.ShowNewFolderButton = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  Write-Output $dialog.SelectedPath
}
"#;
        run_windows_dialog_script(script, "目录选择窗口")
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台暂不支持选择数据目录".to_string())
    }
}

fn pick_save_zip_file_with_windows_dialog() -> Result<Option<std::path::PathBuf>, String> {
    #[cfg(target_os = "windows")]
    {
        let default_name = format!(
            "clipstash-export-{}.zip",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        );
        let script = format!(
            r#"
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$dialog = New-Object System.Windows.Forms.SaveFileDialog
$dialog.Title = '导出 ClipStash 数据'
$dialog.Filter = 'ClipStash 数据包 (*.zip)|*.zip'
$dialog.FileName = '{default_name}'
$dialog.DefaultExt = 'zip'
$dialog.AddExtension = $true
$dialog.OverwritePrompt = $true
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
  Write-Output $dialog.FileName
}}
"#
        );
        run_windows_dialog_script(&script, "导出数据保存窗口")
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台暂不支持选择导出位置".to_string())
    }
}

fn pick_open_zip_file_with_windows_dialog() -> Result<Option<std::path::PathBuf>, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = '导入 ClipStash 数据'
$dialog.Filter = 'ClipStash 数据包 (*.zip)|*.zip'
$dialog.CheckFileExists = $true
$dialog.Multiselect = $false
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  Write-Output $dialog.FileName
}
"#;
        run_windows_dialog_script(script, "导入数据选择窗口")
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台暂不支持选择导入数据包".to_string())
    }
}

#[cfg(target_os = "windows")]
fn run_windows_dialog_script(
    script: &str,
    label: &str,
) -> Result<Option<std::path::PathBuf>, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", script])
        .output()
        .map_err(|err| format!("打开{label}失败：{err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("{label}异常退出")
        } else {
            stderr
        });
    }
    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(std::path::PathBuf::from(selected)))
    }
}
#[cfg(target_os = "windows")]
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(
        app,
        TRAY_SHOW_HIDE_ID,
        "显示/隐藏主窗口",
        true,
        None::<&str>,
    )?;
    let open_data_dir = MenuItem::with_id(
        app,
        TRAY_OPEN_DATA_DIR_ID,
        "打开数据目录",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_hide, &open_data_dir, &quit])?;

    let icon = app.default_window_icon().cloned();
    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("ClipStash Next")
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW_HIDE_ID => toggle_main_window(app),
            TRAY_OPEN_DATA_DIR_ID => open_app_data_dir(),
            TRAY_QUIT_ID => {
                EXIT_REQUESTED.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => show_main_window(tray.app_handle()),
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
            _ => {}
        });

    if let Some(icon) = icon {
        tray = tray.icon(icon);
    }

    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .manage(GlobalShortcutStatus::default())
        .plugin(tauri_plugin_opener::init());

    #[cfg(target_os = "windows")]
    {
        builder = builder
            .plugin(
                tauri_plugin_autostart::Builder::new()
                    .app_name("ClipStash Next")
                    .build(),
            )
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                show_main_window(app);
            }));
    }

    builder
        .setup(|app| {
            #[cfg(not(target_os = "windows"))]
            {
                let data_dir = app
                    .path()
                    .app_data_dir()
                    .map_err(|err| format!("定位应用数据目录失败：{err}"))?;
                app_data::set_app_data_base_dir(data_dir);
            }
            app_data::ensure_app_data_ready()?;
            #[cfg(target_os = "windows")]
            {
                window_targets::start_foreground_tracker();
                setup_tray(app)?;
                setup_global_shortcuts(app);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(target_os = "windows")]
            {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    if window.label() == "main" && !EXIT_REQUESTED.load(Ordering::SeqCst) {
                        let should_hide_to_tray = app_settings::read_settings()
                            .map(|settings| settings.close_to_tray)
                            .unwrap_or(true);
                        if should_hide_to_tray {
                            api.prevent_close();
                            let _ = window.hide();
                        } else {
                            EXIT_REQUESTED.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = window;
                let _ = event;
            }
        })
        .invoke_handler(tauri::generate_handler![
            capture_current_clipboard,
            create_legacy_image_message,
            create_legacy_mixed_message,
            create_legacy_text_message,
            download_and_open_update_installer,
            export_normal_data_zip,
            export_normal_data_zip_bytes,
            fetch_latest_github_release,
            copy_legacy_image_to_clipboard,
            copy_legacy_message_text_to_clipboard,
            copy_legacy_message_import_queue_item_to_clipboard,
            delete_legacy_message,
            get_global_shortcut_errors,
            get_app_settings,
            get_launch_on_startup,
            get_legacy_stats,
            import_data_zip,
            import_data_zip_bytes,
            import_data_zip_from_path,
            list_external_window_targets,
            list_legacy_messages,
            migrate_legacy_data,
            move_app_data_to_selected_dir,
            preview_data_zip,
            open_app_path,
            paste_legacy_import_queue,
            paste_legacy_import_queue_to_recent_window,
            paste_legacy_import_queue_with_optional_archive,
            paste_legacy_import_queue_item,
            preview_legacy_message_import_queue,
            read_dropped_file_bytes,
            read_legacy_image_bytes,
            read_current_clipboard,
            repair_app_data_dir,
            replace_legacy_message_images,
            set_legacy_message_archived,
            set_launch_on_startup,
            stage_legacy_message_import_to_clipboard,
            update_legacy_message_text,
            update_app_settings,
            validate_external_window_target
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn downloaded_installers_open_without_special_update_args() {
        let exe = std::path::Path::new("ClipStash Next_2.0.7_x64-setup.exe");
        let msi = std::path::Path::new("ClipStash Next_2.0.7_x64_en-US.msi");

        assert_eq!(
            exe.extension().and_then(|value| value.to_str()),
            Some("exe")
        );
        assert_eq!(
            msi.extension().and_then(|value| value.to_str()),
            Some("msi")
        );
    }
}
