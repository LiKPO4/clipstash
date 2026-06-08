use serde::Serialize;
use std::time::Duration;

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

#[derive(Serialize)]
pub struct LegacyImportQueuePasteResult {
    pub message_id: i64,
    pub target: window_targets::ExternalWindowTarget,
    pub requested_delay_ms: u64,
    pub completed_count: usize,
    pub failed_item_index: Option<usize>,
    pub failure: Option<String>,
    pub items: Vec<LegacyImportPasteResult>,
}

#[derive(Serialize)]
pub struct LegacyImportQueuePasteArchiveResult {
    pub paste: LegacyImportQueuePasteResult,
    pub archive_requested: bool,
    pub archive_result: Option<legacy_data::LegacyArchiveMessageResult>,
    pub archive_error: Option<String>,
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

pub fn paste_legacy_import_queue(
    message_id: i64,
    target_hwnd: isize,
    delay_ms: Option<u64>,
) -> Result<LegacyImportQueuePasteResult, String> {
    let preview = legacy_data::preview_legacy_message_import_queue(message_id)?;
    if preview.item_count == 0 {
        return Err(format!("粘贴导入队列失败，队列为空：#{message_id}"));
    }

    let requested_delay_ms = delay_ms.unwrap_or(250);
    let delay = Duration::from_millis(requested_delay_ms);
    let mut items = Vec::with_capacity(preview.item_count);
    let mut last_target = None;

    for item_index in 0..preview.item_count {
        if item_index > 0 && !delay.is_zero() {
            std::thread::sleep(delay);
        }

        match paste_legacy_import_queue_item(message_id, item_index, target_hwnd) {
            Ok(result) => {
                last_target = Some(window_targets::ExternalWindowTarget {
                    hwnd: result.target.hwnd,
                    process_id: result.target.process_id,
                    title: result.target.title.clone(),
                });
                items.push(result);
            }
            Err(err) => {
                let target = last_target.unwrap_or_else(|| window_targets::ExternalWindowTarget {
                    hwnd: target_hwnd,
                    process_id: 0,
                    title: String::new(),
                });
                return Ok(LegacyImportQueuePasteResult {
                    message_id,
                    target,
                    requested_delay_ms,
                    completed_count: items.len(),
                    failed_item_index: Some(item_index),
                    failure: Some(err),
                    items,
                });
            }
        }
    }

    let target =
        last_target.ok_or_else(|| format!("粘贴导入队列失败，未执行任何项：#{message_id}"))?;
    Ok(LegacyImportQueuePasteResult {
        message_id,
        target,
        requested_delay_ms,
        completed_count: items.len(),
        failed_item_index: None,
        failure: None,
        items,
    })
}

pub fn paste_legacy_import_queue_with_optional_archive(
    message_id: i64,
    target_hwnd: isize,
    delay_ms: Option<u64>,
    archive_after_success: bool,
) -> Result<LegacyImportQueuePasteArchiveResult, String> {
    let paste = paste_legacy_import_queue(message_id, target_hwnd, delay_ms)?;
    if !archive_after_success || paste.failure.is_some() {
        return Ok(LegacyImportQueuePasteArchiveResult {
            paste,
            archive_requested: archive_after_success,
            archive_result: None,
            archive_error: None,
        });
    }

    match legacy_data::set_legacy_message_archived(message_id, true) {
        Ok(archive_result) => Ok(LegacyImportQueuePasteArchiveResult {
            paste,
            archive_requested: true,
            archive_result: Some(archive_result),
            archive_error: None,
        }),
        Err(err) => Ok(LegacyImportQueuePasteArchiveResult {
            paste,
            archive_requested: true,
            archive_result: None,
            archive_error: Some(err),
        }),
    }
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

    #[test]
    #[ignore = "copies each queue item, focuses a local desktop window, and sends Ctrl+V; set CLIPSTASH_NEXT_PASTE_QUEUE_ID, CLIPSTASH_NEXT_PASTE_QUEUE_HWND, and optionally CLIPSTASH_NEXT_PASTE_QUEUE_DELAY_MS"]
    fn manual_pastes_legacy_import_queue_to_external_window() {
        let message_id = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_ID")
            .expect("set CLIPSTASH_NEXT_PASTE_QUEUE_ID")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_PASTE_QUEUE_ID must be an integer");
        let target_hwnd = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_HWND")
            .expect("set CLIPSTASH_NEXT_PASTE_QUEUE_HWND")
            .parse::<isize>()
            .expect("CLIPSTASH_NEXT_PASTE_QUEUE_HWND must be an integer");
        let delay_ms = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_DELAY_MS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .expect("CLIPSTASH_NEXT_PASTE_QUEUE_DELAY_MS must be an integer")
            });

        let result = paste_legacy_import_queue(message_id, target_hwnd, delay_ms)
            .expect("paste legacy import queue");

        assert_eq!(result.message_id, message_id);
        assert_eq!(result.target.hwnd, target_hwnd);
        assert_eq!(result.failed_item_index, None);
        assert_eq!(result.failure, None);
        assert_eq!(result.completed_count, result.items.len());
        eprintln!(
            "legacy-import-queue-paste-ok message_id={} completed_count={} delay_ms={} target_hwnd={} target_title={}",
            result.message_id,
            result.completed_count,
            result.requested_delay_ms,
            result.target.hwnd,
            result.target.title
        );
    }

    #[test]
    #[ignore = "pastes the full queue and optionally archives after success; set CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_ID, CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_HWND, CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_AFTER_SUCCESS, and optionally CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_DELAY_MS"]
    fn manual_pastes_legacy_import_queue_with_optional_archive() {
        let message_id = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_ID")
            .expect("set CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_ID")
            .parse::<i64>()
            .expect("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_ID must be an integer");
        let target_hwnd = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_HWND")
            .expect("set CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_HWND")
            .parse::<isize>()
            .expect("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_HWND must be an integer");
        let archive_after_success =
            std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_AFTER_SUCCESS")
                .expect("set CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_AFTER_SUCCESS")
                == "1";
        let delay_ms = std::env::var("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_DELAY_MS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .expect("CLIPSTASH_NEXT_PASTE_QUEUE_ARCHIVE_DELAY_MS must be an integer")
            });

        let result = paste_legacy_import_queue_with_optional_archive(
            message_id,
            target_hwnd,
            delay_ms,
            archive_after_success,
        )
        .expect("paste legacy import queue with optional archive");

        assert_eq!(result.paste.message_id, message_id);
        assert_eq!(result.archive_requested, archive_after_success);
        if archive_after_success {
            assert!(result.archive_error.is_none());
            assert!(result.archive_result.is_some());
        } else {
            assert!(result.archive_result.is_none());
        }
        eprintln!(
            "legacy-import-queue-paste-archive-ok message_id={} completed_count={} archive_requested={} archived={}",
            result.paste.message_id,
            result.paste.completed_count,
            result.archive_requested,
            result.archive_result.is_some()
        );
    }
}
