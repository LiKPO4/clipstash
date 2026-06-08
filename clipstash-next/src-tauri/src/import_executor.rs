use serde::Serialize;

use crate::{keyboard_input, legacy_data, window_targets};

#[derive(Serialize)]
pub struct LegacyImportPasteResult {
    pub message_id: i64,
    pub item_index: usize,
    pub staged_kind: String,
    pub text_length: usize,
    pub image_filename: Option<String>,
    pub target: window_targets::ExternalWindowTarget,
    pub sent_ctrl_v: bool,
}

pub fn paste_legacy_import_queue_item(
    message_id: i64,
    item_index: usize,
    target_hwnd: isize,
) -> Result<LegacyImportPasteResult, String> {
    window_targets::validate_external_window_target(target_hwnd)?;
    let copied =
        legacy_data::copy_legacy_message_import_queue_item_to_clipboard(message_id, item_index)?;
    let focused = window_targets::focus_external_window_target(target_hwnd)?;
    keyboard_input::send_ctrl_v()?;

    Ok(LegacyImportPasteResult {
        message_id: copied.message_id,
        item_index: copied.item_index,
        staged_kind: copied.staged_kind,
        text_length: copied.text_length,
        image_filename: copied.image_filename,
        target: focused.target,
        sent_ctrl_v: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "copies a queue item, focuses a local desktop window, and sends Ctrl+V; set CLIPSTASH_NEXT_PASTE_IMPORT_ID, CLIPSTASH_NEXT_PASTE_IMPORT_INDEX, and CLIPSTASH_NEXT_PASTE_IMPORT_HWND"]
    fn manual_pastes_legacy_import_queue_item_to_external_window() {
        let message_id = std::env::var("CLIPSTASH_NEXT_PASTE_IMPORT_ID")
            .expect("set CLIPSTASH_NEXT_PASTE_IMPORT_ID")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_PASTE_IMPORT_ID must be an integer");
        let item_index = std::env::var("CLIPSTASH_NEXT_PASTE_IMPORT_INDEX")
            .expect("set CLIPSTASH_NEXT_PASTE_IMPORT_INDEX")
            .parse::<usize>()
            .expect("CLIPSTASH_NEXT_PASTE_IMPORT_INDEX must be an integer");
        let target_hwnd = std::env::var("CLIPSTASH_NEXT_PASTE_IMPORT_HWND")
            .expect("set CLIPSTASH_NEXT_PASTE_IMPORT_HWND")
            .parse::<isize>()
            .expect("CLIPSTASH_NEXT_PASTE_IMPORT_HWND must be an integer");

        let result = paste_legacy_import_queue_item(message_id, item_index, target_hwnd)
            .expect("paste legacy import queue item");

        assert!(result.sent_ctrl_v);
        assert_eq!(result.message_id, message_id);
        assert_eq!(result.item_index, item_index);
        assert_eq!(result.target.hwnd, target_hwnd);
        eprintln!(
            "legacy-import-paste-ok message_id={} item_index={} kind={} target_hwnd={} target_title={}",
            result.message_id,
            result.item_index,
            result.staged_kind,
            result.target.hwnd,
            result.target.title
        );
    }
}
