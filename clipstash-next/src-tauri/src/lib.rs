mod app_data;
mod app_settings;
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

use arboard::Clipboard;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);
const TRAY_SHOW_HIDE_ID: &str = "tray_show_hide";
const TRAY_OPEN_DATA_DIR_ID: &str = "tray_open_data_dir";
const TRAY_QUIT_ID: &str = "tray_quit";
const SHOW_HIDE_SHORTCUT: &str = "Ctrl+Shift+V";
const CAPTURE_CLIPBOARD_SHORTCUT: &str = "Ctrl+Alt+V";

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

#[derive(Default)]
struct GlobalShortcutStatus {
    errors: Mutex<Vec<String>>,
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
    app.autolaunch()
        .is_enabled()
        .map_err(|err| format!("读取开机自启动状态失败：{err}"))
}

#[tauri::command]
fn set_launch_on_startup(app: AppHandle, enabled: bool) -> Result<bool, String> {
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

#[tauri::command]
fn get_app_settings() -> Result<app_settings::AppSettings, String> {
    app_settings::read_settings()
}

#[tauri::command]
fn update_app_settings(
    patch: app_settings::AppSettingsPatch,
) -> Result<app_settings::AppSettings, String> {
    app_settings::update_settings(patch)
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

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

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

fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }

    let shortcut_text = shortcut.into_string();
    if shortcut_text == SHOW_HIDE_SHORTCUT {
        toggle_main_window(app);
    } else if shortcut_text == CAPTURE_CLIPBOARD_SHORTCUT {
        let _ = capture_current_clipboard_to_app_data();
        show_main_window(app);
    }
}

fn setup_global_shortcuts(app: &tauri::App) {
    let shortcuts = app.global_shortcut();
    let mut errors = Vec::new();

    if let Err(err) = shortcuts.on_shortcuts(
        [SHOW_HIDE_SHORTCUT, CAPTURE_CLIPBOARD_SHORTCUT],
        handle_global_shortcut,
    ) {
        errors.push(format!("快捷键注册失败：{err}"));
    }

    if let Some(status) = app.try_state::<GlobalShortcutStatus>() {
        *status.errors.lock().unwrap() = errors;
    }
}

fn open_app_data_dir() {
    if let Ok(stats) = app_data::read_app_stats() {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(stats.data_dir)
                .spawn();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = open::that(stats.data_dir);
        }
    }
}

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
    tauri::Builder::default()
        .manage(GlobalShortcutStatus::default())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("ClipStash Next")
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .setup(|app| {
            app_data::ensure_app_data_ready()?;
            window_targets::start_foreground_tracker();
            setup_tray(app)?;
            setup_global_shortcuts(app);
            Ok(())
        })
        .on_window_event(|window, event| {
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
        })
        .invoke_handler(tauri::generate_handler![
            capture_current_clipboard,
            create_legacy_image_message,
            create_legacy_mixed_message,
            create_legacy_text_message,
            copy_legacy_image_to_clipboard,
            copy_legacy_message_text_to_clipboard,
            copy_legacy_message_import_queue_item_to_clipboard,
            delete_legacy_message,
            get_global_shortcut_errors,
            get_app_settings,
            get_launch_on_startup,
            get_legacy_stats,
            list_external_window_targets,
            list_legacy_messages,
            migrate_legacy_data,
            paste_legacy_import_queue,
            paste_legacy_import_queue_to_recent_window,
            paste_legacy_import_queue_with_optional_archive,
            paste_legacy_import_queue_item,
            preview_legacy_message_import_queue,
            read_current_clipboard,
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
