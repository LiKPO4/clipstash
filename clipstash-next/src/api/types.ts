export type LegacyStats = {
  data_dir: string;
  db_path: string;
  images_dir: string;
  db_exists: boolean;
  images_dir_exists: boolean;
  normal_count: number;
  archived_count: number;
  total_count: number;
};

export type ExternalWindowTarget = {
  hwnd: number;
  process_id: number;
  title: string;
};

export type ExternalWindowValidation = {
  valid: boolean;
  target: ExternalWindowTarget | null;
};

export type MessageView = "normal" | "archived";

export type SortOrder = "newest" | "oldest";

export type LegacyMessageImage = {
  id: number;
  filename: string;
  path: string;
  exists: boolean;
};

export type LegacyMessage = {
  id: number;
  text_content: string | null;
  created_at: string;
  archived: boolean;
  archived_at: string | null;
  images: LegacyMessageImage[];
};

export type LegacyMessagePage = {
  view: MessageView;
  sort: SortOrder;
  offset: number;
  limit: number;
  total_count: number;
  has_more: boolean;
  messages: LegacyMessage[];
};

export type LegacyDbBackup = {
  source_path: string;
  backup_path: string;
  bytes_copied: number;
};

export type LegacyCreateTextMessageResult = {
  backup: LegacyDbBackup;
  message: LegacyMessage;
};

export type LegacyCreateImageMessageResult = LegacyCreateTextMessageResult;

export type LegacyCreateMixedMessageResult = LegacyCreateTextMessageResult;

export type LegacyUpdateMessageResult = LegacyCreateTextMessageResult;

export type LegacyImageFilesBackup = {
  backup_dir: string;
  filenames: string[];
};

export type LegacyReplaceImagesResult = {
  backup: LegacyDbBackup;
  image_backup: LegacyImageFilesBackup | null;
  message: LegacyMessage;
};

export type LegacyDeleteMessageResult = LegacyReplaceImagesResult;

export type LegacyArchiveMessageResult = LegacyCreateTextMessageResult;

export type LegacyCopyImageResult = {
  filename: string;
  path: string;
  width: number;
  height: number;
};

export type LegacyCopyTextResult = {
  message_id: number;
  text_length: number;
};

export type LegacyImportStageResult = {
  message_id: number;
  staged_kind: "text" | "image";
  text_length: number;
  image_count: number;
  first_image_filename: string | null;
  copied_image: LegacyCopyImageResult | null;
};

export type LegacyImportQueueItem = {
  kind: "text" | "image";
  text: string | null;
  text_length: number;
  image: LegacyMessageImage | null;
};

export type LegacyImportQueuePreview = {
  message_id: number;
  item_count: number;
  text_length: number;
  image_count: number;
  skipped_missing_image_count: number;
  items: LegacyImportQueueItem[];
};

export type LegacyImportQueueCopyResult = {
  message_id: number;
  item_index: number;
  staged_kind: "text" | "image";
  text_length: number;
  image_filename: string | null;
  copied_image: LegacyCopyImageResult | null;
};

export type LegacyImportPasteResult = {
  message_id: number;
  item_index: number;
  staged_kind: "text" | "image";
  text_length: number;
  image_filename: string | null;
  target: ExternalWindowTarget;
  sent_ctrl_v: boolean;
};

export type LegacyImportQueuePasteResult = {
  message_id: number;
  target: ExternalWindowTarget;
  requested_delay_ms: number;
  completed_count: number;
  failed_item_index: number | null;
  failure: string | null;
  items: LegacyImportPasteResult[];
};

export type LegacyImportQueuePasteArchiveResult = {
  paste: LegacyImportQueuePasteResult;
  archive_requested: boolean;
  archive_result: LegacyArchiveMessageResult | null;
  archive_error: string | null;
};
