import { invoke } from "@tauri-apps/api/core";
import type { LegacyMessagePage, LegacyStats, MessageView, SortOrder } from "./types";

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
