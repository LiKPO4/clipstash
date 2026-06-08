import { invoke } from "@tauri-apps/api/core";
import type { LegacyStats } from "./types";

export function getLegacyStats() {
  return invoke<LegacyStats>("get_legacy_stats");
}
