use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, Serialize)]
pub struct ExternalWindowTarget {
    pub hwnd: isize,
    pub process_id: u32,
    pub title: String,
}

#[derive(Serialize)]
pub struct ExternalWindowValidation {
    pub valid: bool,
    pub target: Option<ExternalWindowTarget>,
}

#[derive(Serialize)]
pub struct ExternalWindowFocus {
    pub focused: bool,
    pub target: ExternalWindowTarget,
}

static LAST_EXTERNAL_WINDOW: OnceLock<Mutex<Option<ExternalWindowTarget>>> = OnceLock::new();
static TRACKER_STARTED: OnceLock<()> = OnceLock::new();

fn last_external_window_store() -> &'static Mutex<Option<ExternalWindowTarget>> {
    LAST_EXTERNAL_WINDOW.get_or_init(|| Mutex::new(None))
}

fn remember_external_window(target: ExternalWindowTarget) {
    if let Ok(mut stored) = last_external_window_store().lock() {
        *stored = Some(target);
    }
}

pub fn last_external_window_target() -> Option<ExternalWindowTarget> {
    last_external_window_store().lock().ok()?.clone()
}

#[cfg(target_os = "windows")]
pub fn start_foreground_tracker() {
    TRACKER_STARTED.get_or_init(|| {
        thread::spawn(|| loop {
            if let Some(target) = windows_impl::current_foreground_external_target() {
                remember_external_window(target);
            }
            thread::sleep(Duration::from_millis(500));
        });
    });
}

#[cfg(not(target_os = "windows"))]
pub fn start_foreground_tracker() {}

#[cfg(target_os = "windows")]
pub fn list_external_window_targets() -> Result<Vec<ExternalWindowTarget>, String> {
    windows_impl::list_external_window_targets()
}

#[cfg(not(target_os = "windows"))]
pub fn list_external_window_targets() -> Result<Vec<ExternalWindowTarget>, String> {
    Ok(Vec::new())
}

#[cfg(target_os = "windows")]
pub fn validate_external_window_target(hwnd: isize) -> Result<ExternalWindowValidation, String> {
    windows_impl::validate_external_window_target(hwnd)
}

#[cfg(not(target_os = "windows"))]
pub fn validate_external_window_target(_hwnd: isize) -> Result<ExternalWindowValidation, String> {
    Err("目标窗口校验仅支持 Windows".to_string())
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn focus_external_window_target(hwnd: isize) -> Result<ExternalWindowFocus, String> {
    windows_impl::focus_external_window_target(hwnd)
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn focus_external_window_target(_hwnd: isize) -> Result<ExternalWindowFocus, String> {
    Err("目标窗口聚焦仅支持 Windows".to_string())
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::remember_external_window;
    use super::{ExternalWindowFocus, ExternalWindowTarget, ExternalWindowValidation};
    use std::thread;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    pub fn list_external_window_targets() -> Result<Vec<ExternalWindowTarget>, String> {
        let mut windows = Vec::new();
        let current_process_id = unsafe { GetCurrentProcessId() };
        let mut state = EnumState {
            windows: &mut windows,
            current_process_id,
        };
        let state_ptr = &mut state as *mut EnumState as LPARAM;

        let ok = unsafe { EnumWindows(Some(enum_window_proc), state_ptr) };
        if ok == 0 {
            return Err("枚举外部窗口失败".to_string());
        }

        windows.sort_by(|left, right| {
            left.title
                .to_lowercase()
                .cmp(&right.title.to_lowercase())
                .then(left.hwnd.cmp(&right.hwnd))
        });
        Ok(windows)
    }

    pub fn validate_external_window_target(
        hwnd_value: isize,
    ) -> Result<ExternalWindowValidation, String> {
        if hwnd_value == 0 {
            return Err("目标窗口校验失败，hwnd 不能为空".to_string());
        }

        let hwnd = hwnd_value as HWND;
        let current_process_id = unsafe { GetCurrentProcessId() };
        if unsafe { !is_candidate_window(hwnd, current_process_id) } {
            return Err(format!("目标窗口不可用或属于当前进程：hwnd={hwnd_value}"));
        }

        let target = unsafe { target_from_hwnd(hwnd) }?;
        if target.title.trim().is_empty() {
            return Err(format!("目标窗口标题为空：hwnd={hwnd_value}"));
        }
        remember_external_window(target.clone());

        Ok(ExternalWindowValidation {
            valid: true,
            target: Some(target),
        })
    }

    pub fn focus_external_window_target(hwnd_value: isize) -> Result<ExternalWindowFocus, String> {
        let validation = validate_external_window_target(hwnd_value)?;
        let target = validation
            .target
            .ok_or_else(|| format!("目标窗口校验未返回窗口信息：hwnd={hwnd_value}"))?;
        let hwnd = target.hwnd as HWND;

        unsafe {
            restore_and_raise_window(hwnd);
            if !try_set_foreground_window(hwnd) {
                return Err(format!("目标窗口聚焦失败：hwnd={hwnd_value}"));
            }

            thread::sleep(Duration::from_millis(120));
            if GetForegroundWindow() != hwnd {
                return Err(format!("目标窗口未成为前台窗口：hwnd={hwnd_value}"));
            }
        }

        Ok(ExternalWindowFocus {
            focused: true,
            target,
        })
    }

    pub fn current_foreground_external_target() -> Option<ExternalWindowTarget> {
        unsafe {
            let hwnd = GetForegroundWindow();
            let current_process_id = GetCurrentProcessId();
            if hwnd.is_null() || !is_candidate_window(hwnd, current_process_id) {
                return None;
            }
            let target = target_from_hwnd(hwnd).ok()?;
            if target.title.trim().is_empty() {
                return None;
            }
            Some(target)
        }
    }

    unsafe fn restore_and_raise_window(hwnd: HWND) {
        ShowWindow(hwnd, SW_RESTORE);
        BringWindowToTop(hwnd);
        SetActiveWindow(hwnd);
        SetFocus(hwnd);
    }

    unsafe fn try_set_foreground_window(hwnd: HWND) -> bool {
        if try_raise_without_attach(hwnd) {
            return true;
        }

        let current_thread_id = GetCurrentThreadId();
        let foreground = GetForegroundWindow();
        let foreground_thread_id = if foreground.is_null() {
            0
        } else {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        };
        let target_thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());

        let attach_foreground =
            foreground_thread_id != 0 && foreground_thread_id != current_thread_id;
        let attach_target = target_thread_id != 0 && target_thread_id != current_thread_id;

        if attach_foreground {
            AttachThreadInput(current_thread_id, foreground_thread_id, 1);
        }
        if attach_target {
            AttachThreadInput(current_thread_id, target_thread_id, 1);
        }

        let focused = try_raise_without_attach(hwnd);

        if attach_target {
            AttachThreadInput(current_thread_id, target_thread_id, 0);
        }
        if attach_foreground {
            AttachThreadInput(current_thread_id, foreground_thread_id, 0);
        }

        focused
    }

    unsafe fn try_raise_without_attach(hwnd: HWND) -> bool {
        for _ in 0..5 {
            restore_and_raise_window(hwnd);
            if SetForegroundWindow(hwnd) != 0 || GetForegroundWindow() == hwnd {
                return true;
            }
            thread::sleep(Duration::from_millis(80));
        }
        GetForegroundWindow() == hwnd
    }

    struct EnumState<'a> {
        windows: &'a mut Vec<ExternalWindowTarget>,
        current_process_id: u32,
    }

    unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let state = &mut *(lparam as *mut EnumState);
        if !is_candidate_window(hwnd, state.current_process_id) {
            return 1;
        }

        let title = window_title(hwnd);
        if title.trim().is_empty() {
            return 1;
        }

        let mut process_id = 0_u32;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        state.windows.push(ExternalWindowTarget {
            hwnd: hwnd as isize,
            process_id,
            title,
        });
        1
    }

    unsafe fn is_candidate_window(hwnd: HWND, current_process_id: u32) -> bool {
        if IsWindow(hwnd) == 0 || IsWindowVisible(hwnd) == 0 {
            return false;
        }

        let mut process_id = 0_u32;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        process_id != 0 && process_id != current_process_id
    }

    unsafe fn target_from_hwnd(hwnd: HWND) -> Result<ExternalWindowTarget, String> {
        let mut process_id = 0_u32;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        let title = window_title(hwnd);
        Ok(ExternalWindowTarget {
            hwnd: hwnd as isize,
            process_id,
            title,
        })
    }

    unsafe fn window_title(hwnd: HWND) -> String {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return String::new();
        }

        let mut buffer = vec![0_u16; (length + 1) as usize];
        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if copied <= 0 {
            return String::new();
        }

        String::from_utf16_lossy(&buffer[..copied as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "reads local desktop windows; set CLIPSTASH_NEXT_LIST_WINDOWS"]
    fn manual_lists_external_window_targets() {
        std::env::var("CLIPSTASH_NEXT_LIST_WINDOWS")
            .expect("set CLIPSTASH_NEXT_LIST_WINDOWS to list local windows");

        let windows = list_external_window_targets().expect("list external windows");
        for window in &windows {
            eprintln!(
                "external-window hwnd={} pid={} title={}",
                window.hwnd, window.process_id, window.title
            );
        }

        eprintln!("external-window-count={}", windows.len());
    }

    #[test]
    #[ignore = "reads local desktop windows; set CLIPSTASH_NEXT_VALIDATE_WINDOW"]
    fn manual_validates_external_window_target() {
        std::env::var("CLIPSTASH_NEXT_VALIDATE_WINDOW")
            .expect("set CLIPSTASH_NEXT_VALIDATE_WINDOW to validate the first local window");

        let windows = list_external_window_targets().expect("list external windows");
        let first = windows
            .first()
            .expect("at least one external window is required for manual validation");
        let validation =
            validate_external_window_target(first.hwnd).expect("validate external window");
        let target = validation.target.expect("validated target");

        assert!(validation.valid);
        assert_eq!(target.hwnd, first.hwnd);
        assert_eq!(target.process_id, first.process_id);
        assert!(!target.title.trim().is_empty());

        eprintln!(
            "external-window-validate-ok hwnd={} pid={} title={}",
            target.hwnd, target.process_id, target.title
        );
    }

    #[test]
    #[ignore = "focuses a local desktop window; set CLIPSTASH_NEXT_FOCUS_WINDOW"]
    fn manual_focuses_external_window_target() {
        let requested = std::env::var("CLIPSTASH_NEXT_FOCUS_WINDOW")
            .expect("set CLIPSTASH_NEXT_FOCUS_WINDOW to a hwnd, or any value to focus the first local window");
        let hwnd = match requested.parse::<isize>() {
            Ok(hwnd) => hwnd,
            Err(_) => {
                list_external_window_targets()
                    .expect("list external windows")
                    .first()
                    .expect("at least one external window is required for manual focus")
                    .hwnd
            }
        };

        let focused = focus_external_window_target(hwnd).expect("focus external window");

        assert!(focused.focused);
        assert_eq!(focused.target.hwnd, hwnd);
        eprintln!(
            "external-window-focus-ok hwnd={} pid={} title={}",
            focused.target.hwnd, focused.target.process_id, focused.target.title
        );
    }
}
