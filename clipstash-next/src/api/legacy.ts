import { invoke } from "@tauri-apps/api/core";
import type {
  LegacyCreateImageMessageResult,
  LegacyCreateTextMessageResult,
  LegacyMessagePage,
  LegacyStats,
  MessageView,
  SortOrder,
} from "./types";

export function getLegacyStats() {
  return invoke<LegacyStats>("get_legacy_stats");
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
