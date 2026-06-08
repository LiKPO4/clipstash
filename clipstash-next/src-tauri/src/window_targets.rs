use serde::Serialize;

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
    use super::{ExternalWindowFocus, ExternalWindowTarget, ExternalWindowValidation};
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
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
            ShowWindow(hwnd, SW_RESTORE);
            if SetForegroundWindow(hwnd) == 0 {
                return Err(format!("目标窗口聚焦失败：hwnd={hwnd_value}"));
            }

            let foreground = GetForegroundWindow();
            if foreground != hwnd {
                return Err(format!("目标窗口未成为前台窗口：hwnd={hwnd_value}"));
            }
        }

        Ok(ExternalWindowFocus {
            focused: true,
            target,
        })
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
