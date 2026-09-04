import { invokeDataTransfer } from "../transferProgress";
import { invoke } from "@tauri-apps/api/core";

export function importAndroidShare(shareId: string) {
  return invoke<LegacyMessage>("import_android_share", { shareId });
}
import type {
  AppMigrationResult,
  AppDataMoveResult,
  AppDataRepairResult,
  AppSettings,
  AppSettingsPatch,
  ClipboardContent,
  DataExportBytesResult,
  DataExportResult,
  DataImportPreview,
  DataImportResult,
  ExternalWindowTarget,
  ExternalWindowValidation,
  GithubReleaseInfo,
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
  LegacyMessage,
  LegacyMessagePage,
  LegacyReplaceImagesResult,
  LegacySplitMessageResult,
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

export function exportNormalDataZip() {
  return invokeDataTransfer<DataExportResult>("export_normal_data_zip");
}

export function exportNormalDataZipBytes() {
  return invokeDataTransfer<DataExportBytesResult>("export_normal_data_zip_bytes");
}

export function exportNormalDataZipFile() {
  return invokeDataTransfer<Omit<DataExportBytesResult, "bytes">>("export_normal_data_zip_file");
}

export function archiveExportedMessages(messageIds: number[]) {
  return invoke<LegacyStats>("archive_exported_messages", { messageIds });
}

export function importDataZip() {
  return invokeDataTransfer<DataImportResult>("import_data_zip");
}

export function previewDataZip() {
  return invokeDataTransfer<DataImportPreview>("preview_data_zip");
}

export function importDataZipBytes(filename: string, bytes: number[]) {
  return invokeDataTransfer<DataImportResult>("import_data_zip_bytes", { filename, bytes });
}

export function importDataZipFromPath(path: string) {
  return invokeDataTransfer<DataImportResult>("import_data_zip_from_path", { path });
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

export function fetchLatestGithubRelease() {
  return invoke<GithubReleaseInfo>("fetch_latest_github_release");
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
  search,
}: {
  view: MessageView;
  sort: SortOrder;
  offset: number;
  limit: number;
  search?: string;
}) {
  const args: {
    view: MessageView;
    sort: SortOrder;
    offset: number;
    limit: number;
    search?: string;
  } = {
    view,
    sort,
    offset,
    limit,
  };
  const normalizedSearch = search?.trim();
  if (normalizedSearch) {
    args.search = normalizedSearch;
  }
  return invoke<LegacyMessagePage>("list_legacy_messages", args);
}

export function getLegacyMessage(messageId: number) {
  return invoke<LegacyMessage>("get_legacy_message", { messageId });
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

export function splitLegacyMessage(
  messageId: number,
  textContent: string,
  imagesData: number[][],
) {
  return invoke<LegacySplitMessageResult>("split_legacy_message", {
    messageId,
    textContent,
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

export async function readLegacyImageBytes(filename: string): Promise<Uint8Array> {
  const bytes = await invoke<ArrayBuffer | Uint8Array>("read_legacy_image_bytes", {
    filename,
  });
  return bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
}

export async function readImageThumbnailBytes(filename: string, expectedPath: string): Promise<Uint8Array> {
  const bytes = await invoke<ArrayBuffer | Uint8Array>("read_image_thumbnail_bytes", { filename, expectedPath });
  return bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
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

export function previewLegacyMessageImportQueue(
  messageId: number,
  matchBlankLinesToImages: boolean,
) {
  return invoke<LegacyImportQueuePreview>("preview_legacy_message_import_queue", {
    messageId,
    matchBlankLinesToImages,
  });
}

export function copyLegacyMessageImportQueueItemToClipboard(
  messageId: number,
  itemIndex: number,
  matchBlankLinesToImages: boolean,
) {
  return invoke<LegacyImportQueueCopyResult>(
    "copy_legacy_message_import_queue_item_to_clipboard",
    {
      messageId,
      itemIndex,
      matchBlankLinesToImages,
    },
  );
}

export function pasteLegacyImportQueueItem(
  messageId: number,
  itemIndex: number,
  targetHwnd: number,
  matchBlankLinesToImages: boolean,
) {
  return invoke<LegacyImportPasteResult>("paste_legacy_import_queue_item", {
    messageId,
    itemIndex,
    targetHwnd,
    matchBlankLinesToImages,
  });
}

export function pasteLegacyImportQueue(
  messageId: number,
  targetHwnd: number,
  delayMs?: number,
  matchBlankLinesToImages = false,
) {
  return invoke<LegacyImportQueuePasteResult>("paste_legacy_import_queue", {
    messageId,
    targetHwnd,
    delayMs,
    matchBlankLinesToImages,
  });
}

export function pasteLegacyImportQueueWithOptionalArchive({
  messageId,
  targetHwnd,
  delayMs,
  archiveAfterSuccess,
  matchBlankLinesToImages,
}: {
  messageId: number;
  targetHwnd: number;
  delayMs?: number;
  archiveAfterSuccess: boolean;
  matchBlankLinesToImages: boolean;
}) {
  return invoke<LegacyImportQueuePasteArchiveResult>(
    "paste_legacy_import_queue_with_optional_archive",
    {
      messageId,
      targetHwnd,
      delayMs,
      archiveAfterSuccess,
      matchBlankLinesToImages,
    },
  );
}

export function pasteLegacyImportQueueToRecentWindow({
  messageId,
  delayMs,
  archiveAfterSuccess,
  matchBlankLinesToImages,
}: {
  messageId: number;
  delayMs?: number;
  archiveAfterSuccess: boolean;
  matchBlankLinesToImages: boolean;
}) {
  return invoke<LegacyImportQueuePasteArchiveResult>(
    "paste_legacy_import_queue_to_recent_window",
    {
      messageId,
      delayMs,
      archiveAfterSuccess,
      matchBlankLinesToImages,
    },
  );
}
