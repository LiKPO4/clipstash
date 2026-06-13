import { invoke } from "@tauri-apps/api/core";
import type {
  AppMigrationResult,
  AppDataMoveResult,
  AppDataRepairResult,
  AppSettings,
  AppSettingsPatch,
  ClipboardContent,
  ExternalWindowTarget,
  ExternalWindowValidation,
  LegacyCreateImageMessageResult,
  LegacyArchiveMessageResult,
  LegacyCreateMixedMessageResult,
  LegacyCreateTextMessageResult,
  LegacyDeleteMessageResult,
  LegacyCopyImageResult,
  LegacyCopyTextResult,
  LegacyImportQueueCopyResult,
  LegacyImportPasteResult,
  LegacyImportQueuePasteResult,
  LegacyImportQueuePasteArchiveResult,
  LegacyImportQueuePreview,
  LegacyImportStageResult,
  LegacyMessagePage,
  LegacyReplaceImagesResult,
  LegacyStats,
  LegacyUpdateMessageResult,
  MessageView,
  SortOrder,
} from "./types";

export function getLegacyStats() {
  return invoke<LegacyStats>("get_legacy_stats");
}

export function migrateLegacyData() {
  return invoke<AppMigrationResult>("migrate_legacy_data");
}

export function moveAppDataToSelectedDir() {
  return invoke<AppDataMoveResult>("move_app_data_to_selected_dir");
}

export function openAppPath(path: string) {
  return invoke<void>("open_app_path", { path });
}

export function repairAppDataDir() {
  return invoke<AppDataRepairResult>("repair_app_data_dir");
}

export function getAppSettings() {
  return invoke<AppSettings>("get_app_settings");
}

export function updateAppSettings(patch: AppSettingsPatch) {
  return invoke<AppSettings>("update_app_settings", { patch });
}

export function downloadAndOpenUpdateInstaller(downloadUrl: string, filename: string) {
  return invoke<{ installer_path: string }>("download_and_open_update_installer", {
    downloadUrl,
    filename,
  });
}

export function getGlobalShortcutErrors() {
  return invoke<string[]>("get_global_shortcut_errors");
}

export function getLaunchOnStartup() {
  return invoke<boolean>("get_launch_on_startup");
}

export function setLaunchOnStartup(enabled: boolean) {
  return invoke<boolean>("set_launch_on_startup", { enabled });
}

export function readCurrentClipboard() {
  return invoke<ClipboardContent>("read_current_clipboard");
}

export function listExternalWindowTargets() {
  return invoke<ExternalWindowTarget[]>("list_external_window_targets");
}

export function validateExternalWindowTarget(hwnd: number) {
  return invoke<ExternalWindowValidation>("validate_external_window_target", {
    hwnd,
  });
}

export function listLegacyMessages({
  view,
  sort,
  offset,
  limit,
}: {
  view: MessageView;
  sort: SortOrder;
  offset: number;
  limit: number;
}) {
  return invoke<LegacyMessagePage>("list_legacy_messages", {
    view,
    sort,
    offset,
    limit,
  });
}

export function createLegacyTextMessage(textContent: string) {
  return invoke<LegacyCreateTextMessageResult>("create_legacy_text_message", {
    textContent,
  });
}

export function createLegacyImageMessage(imagesData: number[][]) {
  return invoke<LegacyCreateImageMessageResult>("create_legacy_image_message", {
    imagesData,
  });
}

export function createLegacyMixedMessage(textContent: string, imagesData: number[][]) {
  return invoke<LegacyCreateMixedMessageResult>("create_legacy_mixed_message", {
    textContent,
    imagesData,
  });
}

export function updateLegacyMessageText(messageId: number, textContent: string | null) {
  return invoke<LegacyUpdateMessageResult>("update_legacy_message_text", {
    messageId,
    textContent,
  });
}

export function replaceLegacyMessageImages(messageId: number, imagesData: number[][]) {
  return invoke<LegacyReplaceImagesResult>("replace_legacy_message_images", {
    messageId,
    imagesData,
  });
}

export function deleteLegacyMessage(messageId: number) {
  return invoke<LegacyDeleteMessageResult>("delete_legacy_message", {
    messageId,
  });
}

export function setLegacyMessageArchived(messageId: number, archived: boolean) {
  return invoke<LegacyArchiveMessageResult>("set_legacy_message_archived", {
    messageId,
    archived,
  });
}

export function copyLegacyImageToClipboard(filename: string) {
  return invoke<LegacyCopyImageResult>("copy_legacy_image_to_clipboard", {
    filename,
  });
}

export function readLegacyImageBytes(filename: string) {
  return invoke<number[]>("read_legacy_image_bytes", {
    filename,
  });
}

export function readDroppedFileBytes(path: string) {
  return invoke<number[]>("read_dropped_file_bytes", {
    path,
  });
}

export function copyLegacyMessageTextToClipboard(messageId: number) {
  return invoke<LegacyCopyTextResult>("copy_legacy_message_text_to_clipboard", {
    messageId,
  });
}

export function stageLegacyMessageImportToClipboard(messageId: number) {
  return invoke<LegacyImportStageResult>("stage_legacy_message_import_to_clipboard", {
    messageId,
  });
}

export function previewLegacyMessageImportQueue(messageId: number) {
  return invoke<LegacyImportQueuePreview>("preview_legacy_message_import_queue", {
    messageId,
  });
}

export function copyLegacyMessageImportQueueItemToClipboard(
  messageId: number,
  itemIndex: number,
) {
  return invoke<LegacyImportQueueCopyResult>(
    "copy_legacy_message_import_queue_item_to_clipboard",
    {
      messageId,
      itemIndex,
    },
  );
}

export function pasteLegacyImportQueueItem(
  messageId: number,
  itemIndex: number,
  targetHwnd: number,
) {
  return invoke<LegacyImportPasteResult>("paste_legacy_import_queue_item", {
    messageId,
    itemIndex,
    targetHwnd,
  });
}

export function pasteLegacyImportQueue(
  messageId: number,
  targetHwnd: number,
  delayMs?: number,
) {
  return invoke<LegacyImportQueuePasteResult>("paste_legacy_import_queue", {
    messageId,
    targetHwnd,
    delayMs,
  });
}

export function pasteLegacyImportQueueWithOptionalArchive({
  messageId,
  targetHwnd,
  delayMs,
  archiveAfterSuccess,
}: {
  messageId: number;
  targetHwnd: number;
  delayMs?: number;
  archiveAfterSuccess: boolean;
}) {
  return invoke<LegacyImportQueuePasteArchiveResult>(
    "paste_legacy_import_queue_with_optional_archive",
    {
      messageId,
      targetHwnd,
      delayMs,
      archiveAfterSuccess,
    },
  );
}

export function pasteLegacyImportQueueToRecentWindow({
  messageId,
  delayMs,
  archiveAfterSuccess,
}: {
  messageId: number;
  delayMs?: number;
  archiveAfterSuccess: boolean;
}) {
  return invoke<LegacyImportQueuePasteArchiveResult>(
    "paste_legacy_import_queue_to_recent_window",
    {
      messageId,
      delayMs,
      archiveAfterSuccess,
    },
  );
}
