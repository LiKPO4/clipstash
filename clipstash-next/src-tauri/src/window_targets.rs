use serde::Serialize;

#[derive(Serialize)]
pub struct ExternalWindowTarget {
    pub hwnd: isize,
    pub process_id: u32,
    pub title: String,
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
mod windows_impl {
    use super::ExternalWindowTarget;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
        IsWindowVisible,
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
}
