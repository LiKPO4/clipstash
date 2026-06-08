import { invoke } from "@tauri-apps/api/core";
import type {
  LegacyCreateImageMessageResult,
  LegacyCreateMixedMessageResult,
  LegacyCreateTextMessageResult,
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
