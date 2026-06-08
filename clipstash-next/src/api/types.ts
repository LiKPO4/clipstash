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
