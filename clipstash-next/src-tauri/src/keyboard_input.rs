#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn send_ctrl_v() -> Result<(), String> {
    windows_impl::send_ctrl_v()
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn send_ctrl_v() -> Result<(), String> {
    Err("发送 Ctrl+V 仅支持 Windows".to_string())
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VK_V: u16 = 0x56;

    pub fn send_ctrl_v() -> Result<(), String> {
        let mut inputs: [INPUT; 4] = unsafe { std::mem::zeroed() };
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[3].r#type = INPUT_KEYBOARD;

        inputs[0].Anonymous.ki.wVk = VK_CONTROL;
        inputs[0].Anonymous.ki.dwFlags = 0;

        inputs[1].Anonymous.ki.wVk = VK_V;
        inputs[1].Anonymous.ki.dwFlags = 0;

        inputs[2].Anonymous.ki.wVk = VK_V;
        inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        inputs[3].Anonymous.ki.wVk = VK_CONTROL;
        inputs[3].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        let sent = unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if sent != inputs.len() as u32 {
            return Err(format!(
                "发送 Ctrl+V 失败，期望发送 {} 个输入事件，实际发送 {sent}",
                inputs.len()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "sends Ctrl+V to the current foreground window; set CLIPSTASH_NEXT_SEND_CTRL_V"]
    fn manual_sends_ctrl_v_to_foreground_window() {
        std::env::var("CLIPSTASH_NEXT_SEND_CTRL_V")
            .expect("set CLIPSTASH_NEXT_SEND_CTRL_V to send Ctrl+V to foreground window");

        send_ctrl_v().expect("send Ctrl+V");
        eprintln!("ctrl-v-send-ok");
    }
}
