use crate::{app_data, legacy_paths};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub always_on_top: bool,
    pub close_to_tray: bool,
    pub launch_on_startup: bool,
    pub main_window_state: Option<MainWindowState>,
    pub archive_after_import: bool,
    pub archive_after_export: bool,
    pub paste_interval_ms: u64,
    pub show_hotkey: String,
    pub capture_hotkey: String,
    pub hover_delay: f64,
    pub scroll_lines: i64,
    pub font_scale: i64,
    pub edit_textarea_height: i64,
    pub sort: String,
    pub message_double_click_action: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MainWindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            close_to_tray: true,
            launch_on_startup: false,
            main_window_state: None,
            archive_after_import: false,
            archive_after_export: false,
            paste_interval_ms: 250,
            show_hotkey: "Ctrl+Shift+V".to_string(),
            capture_hotkey: "Ctrl+Alt+V".to_string(),
            hover_delay: 0.8,
            scroll_lines: 1,
            font_scale: 0,
            edit_textarea_height: 360,
            sort: "newest".to_string(),
            message_double_click_action: "edit".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AppSettingsPatch {
    pub always_on_top: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub launch_on_startup: Option<bool>,
    pub main_window_state: Option<Option<MainWindowState>>,
    pub archive_after_import: Option<bool>,
    pub archive_after_export: Option<bool>,
    pub paste_interval_ms: Option<u64>,
    pub show_hotkey: Option<String>,
    pub capture_hotkey: Option<String>,
    pub hover_delay: Option<f64>,
    pub scroll_lines: Option<i64>,
    pub font_scale: Option<i64>,
    pub edit_textarea_height: Option<i64>,
    pub sort: Option<String>,
    pub message_double_click_action: Option<String>,
}

#[derive(Default, Deserialize)]
struct LegacySettings {
    hover_delay_ms: Option<u64>,
    auto_archive_after_import: Option<bool>,
    launch_on_startup: Option<bool>,
    sort_order: Option<String>,
    show_hotkey: Option<String>,
    capture_hotkey: Option<String>,
    scroll_speed: Option<i64>,
    app_font_size_delta: Option<i64>,
}

pub fn read_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        let settings = read_legacy_settings()
            .map(normalize_settings)
            .unwrap_or_else(AppSettings::default);
        write_settings(&settings)?;
        return Ok(settings);
    }

    let text = fs::read_to_string(&path)
        .map_err(|err| format!("读取设置文件失败：{}：{err}", path.display()))?;
    let settings = serde_json::from_str::<AppSettings>(&text)
        .map(normalize_settings)
        .map_err(|err| format!("解析设置文件失败：{}：{err}", path.display()))?;
    Ok(settings)
}

pub fn update_settings(patch: AppSettingsPatch) -> Result<AppSettings, String> {
    let mut settings = read_settings()?;
    if let Some(value) = patch.always_on_top {
        settings.always_on_top = value;
    }
    if let Some(value) = patch.close_to_tray {
        settings.close_to_tray = value;
    }
    if let Some(value) = patch.launch_on_startup {
        settings.launch_on_startup = value;
    }
    if let Some(value) = patch.main_window_state {
        settings.main_window_state = value;
    }
    if let Some(value) = patch.archive_after_import {
        settings.archive_after_import = value;
    }
    if let Some(value) = patch.archive_after_export {
        settings.archive_after_export = value;
    }
    if let Some(value) = patch.paste_interval_ms {
        settings.paste_interval_ms = value;
    }
    if let Some(value) = patch.show_hotkey {
        settings.show_hotkey = value;
    }
    if let Some(value) = patch.capture_hotkey {
        settings.capture_hotkey = value;
    }
    if let Some(value) = patch.hover_delay {
        settings.hover_delay = value;
    }
    if let Some(value) = patch.scroll_lines {
        settings.scroll_lines = value;
    }
    if let Some(value) = patch.font_scale {
        settings.font_scale = value;
    }
    if let Some(value) = patch.edit_textarea_height {
        settings.edit_textarea_height = value;
    }
    if let Some(value) = patch.sort {
        settings.sort = value;
    }
    if let Some(value) = patch.message_double_click_action {
        settings.message_double_click_action = value;
    }

    let settings = normalize_settings(settings);
    write_settings(&settings)?;
    Ok(settings)
}

fn write_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建设置目录失败：{err}"))?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("序列化设置文件失败：{err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入设置文件失败：{}：{err}", path.display()))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(app_data::app_data_dir_path()?.join(SETTINGS_FILE_NAME))
}

fn read_legacy_settings() -> Option<AppSettings> {
    let path = legacy_paths::legacy_data_dir()
        .ok()?
        .join(SETTINGS_FILE_NAME);
    let text = fs::read_to_string(path).ok()?;
    let legacy = serde_json::from_str::<LegacySettings>(&text).ok()?;
    let mut settings = AppSettings::default();

    if let Some(value) = legacy.hover_delay_ms {
        settings.hover_delay = value as f64 / 1000.0;
    }
    if let Some(value) = legacy.auto_archive_after_import {
        settings.archive_after_import = value;
    }
    if let Some(value) = legacy.launch_on_startup {
        settings.launch_on_startup = value;
    }
    if let Some(value) = legacy.sort_order {
        settings.sort = value;
    }
    if let Some(value) = legacy.show_hotkey {
        settings.show_hotkey = value;
    }
    if let Some(value) = legacy.capture_hotkey {
        settings.capture_hotkey = value;
    }
    if let Some(value) = legacy.scroll_speed {
        settings.scroll_lines = value;
    }
    if let Some(value) = legacy.app_font_size_delta {
        settings.font_scale = value;
    }

    Some(settings)
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    if let Some(state) = settings.main_window_state.take() {
        settings.main_window_state = normalize_main_window_state(state);
    }
    settings.paste_interval_ms = settings.paste_interval_ms.clamp(50, 3000);
    settings.hover_delay = settings.hover_delay.clamp(0.0, 2.0);
    settings.scroll_lines = settings.scroll_lines.clamp(1, 8);
    settings.font_scale = settings.font_scale.clamp(-4, 4);
    settings.edit_textarea_height = settings.edit_textarea_height.clamp(180, 700);
    if settings.sort != "oldest" {
        settings.sort = "newest".to_string();
    }
    if settings.message_double_click_action != "create"
        && settings.message_double_click_action != "none"
    {
        settings.message_double_click_action = "edit".to_string();
    }
    settings.show_hotkey =
        normalize_hotkey(&settings.show_hotkey, &AppSettings::default().show_hotkey);
    settings.capture_hotkey = normalize_hotkey(
        &settings.capture_hotkey,
        &AppSettings::default().capture_hotkey,
    );
    settings
}

fn normalize_main_window_state(state: MainWindowState) -> Option<MainWindowState> {
    let width = state.width.clamp(360, 2400);
    let height = state.height.clamp(600, 2400);
    if state.x.abs() > 100_000 || state.y.abs() > 100_000 {
        return None;
    }
    Some(MainWindowState {
        x: state.x,
        y: state.y,
        width,
        height,
    })
}

fn normalize_hotkey(value: &str, fallback: &str) -> String {
    let parts = value
        .split('+')
        .map(|part| part.trim().trim_matches(['<', '>']))
        .filter(|part| !part.is_empty())
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" | "option" => "Alt".to_string(),
            "cmd" | "command" | "meta" | "super" | "win" | "windows" => "Super".to_string(),
            other if other.len() == 1 => other.to_ascii_uppercase(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        fallback.to_string()
    } else {
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        path::Path,
        sync::{Mutex, OnceLock},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn isolated_appdata(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "clipstash-next-settings-{name}-{}",
            std::process::id()
        ))
    }

    fn reset_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
        fs::create_dir_all(path).unwrap();
    }

    #[test]
    fn reads_missing_settings_fields_with_defaults() {
        let _guard = env_lock().lock().unwrap();
        let appdata = isolated_appdata("missing-fields");
        reset_dir(&appdata);
        let settings_dir = appdata.join("ClipStash Next");
        fs::create_dir_all(&settings_dir).unwrap();
        fs::write(
            settings_dir.join(SETTINGS_FILE_NAME),
            r#"{
  "always_on_top": true,
  "archive_after_import": true,
  "paste_interval_ms": 500,
  "show_hotkey": "<ctrl>+<shift>+v",
  "capture_hotkey": "<ctrl>+<alt>+v",
  "hover_delay": 1.2,
  "scroll_lines": 3,
  "font_scale": 1,
  "sort": "oldest"
}"#,
        )
        .unwrap();

        env::set_var("APPDATA", &appdata);

        let settings = read_settings().unwrap();

        assert!(settings.always_on_top);
        assert!(settings.close_to_tray);
        assert!(!settings.launch_on_startup);
        assert!(settings.main_window_state.is_none());
        assert!(settings.archive_after_import);
        assert!(!settings.archive_after_export);
        assert_eq!(settings.edit_textarea_height, 360);
        assert_eq!(settings.message_double_click_action, "edit");
        assert_eq!(settings.sort, "oldest");
    }

    #[test]
    fn updates_close_to_tray_setting() {
        let _guard = env_lock().lock().unwrap();
        let appdata = isolated_appdata("close-to-tray");
        reset_dir(&appdata);
        env::set_var("APPDATA", &appdata);

        let settings = update_settings(AppSettingsPatch {
            always_on_top: None,
            close_to_tray: Some(false),
            launch_on_startup: None,
            main_window_state: None,
            archive_after_import: None,
            archive_after_export: None,
            message_double_click_action: None,
            paste_interval_ms: None,
            show_hotkey: None,
            capture_hotkey: None,
            hover_delay: None,
            scroll_lines: None,
            font_scale: None,
            edit_textarea_height: None,
            sort: None,
        })
        .unwrap();

        assert!(!settings.close_to_tray);

        let persisted = read_settings().unwrap();
        assert!(!persisted.close_to_tray);
    }

    #[test]
    fn updates_main_window_state_setting() {
        let _guard = env_lock().lock().unwrap();
        let appdata = isolated_appdata("window-state");
        reset_dir(&appdata);
        env::set_var("APPDATA", &appdata);

        let settings = update_settings(AppSettingsPatch {
            main_window_state: Some(Some(MainWindowState {
                x: 120,
                y: 80,
                width: 390,
                height: 740,
            })),
            ..Default::default()
        })
        .unwrap();

        let state = settings.main_window_state.unwrap();
        assert_eq!(state.x, 120);
        assert_eq!(state.y, 80);
        assert_eq!(state.width, 390);
        assert_eq!(state.height, 740);

        let persisted = read_settings().unwrap();
        assert_eq!(persisted.main_window_state.unwrap().x, 120);
    }

    #[test]
    fn updates_archive_after_export_setting() {
        let _guard = env_lock().lock().unwrap();
        let appdata = isolated_appdata("archive-after-export");
        reset_dir(&appdata);
        env::set_var("APPDATA", &appdata);

        let settings = update_settings(AppSettingsPatch {
            archive_after_export: Some(true),
            ..Default::default()
        })
        .unwrap();

        assert!(settings.archive_after_export);
        assert!(read_settings().unwrap().archive_after_export);
    }

    #[test]
    fn migrates_legacy_settings_when_next_settings_do_not_exist() {
        let _guard = env_lock().lock().unwrap();
        let appdata = isolated_appdata("legacy-settings");
        reset_dir(&appdata);
        let legacy_dir = appdata.join("ClipStash");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(
            legacy_dir.join(SETTINGS_FILE_NAME),
            r#"{
  "hover_delay_ms": 1200,
  "auto_archive_after_import": true,
  "sort_order": "oldest",
  "launch_on_startup": true,
  "show_hotkey": "<ctrl>+<shift>+z",
  "capture_hotkey": "<ctrl>+alt+v",
  "scroll_speed": 5,
  "app_font_size_delta": 2
}"#,
        )
        .unwrap();
        env::set_var("APPDATA", &appdata);

        let settings = read_settings().unwrap();

        assert_eq!(settings.hover_delay, 1.2);
        assert!(settings.archive_after_import);
        assert_eq!(settings.sort, "oldest");
        assert_eq!(settings.show_hotkey, "Ctrl+Shift+Z");
        assert_eq!(settings.capture_hotkey, "Ctrl+Alt+V");
        assert_eq!(settings.scroll_lines, 5);
        assert_eq!(settings.font_scale, 2);
        assert!(settings.launch_on_startup);
        assert!(settings.close_to_tray);
        assert!(appdata
            .join("ClipStash Next")
            .join(SETTINGS_FILE_NAME)
            .is_file());
    }
}
