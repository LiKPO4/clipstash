import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";

const { invokeMock, openPathMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openPathMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: openPathMock,
}));

const stats = {
  data_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash",
  db_path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db",
  images_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images",
  db_exists: true,
  images_dir_exists: true,
  normal_count: 6,
  archived_count: 109,
  total_count: 115,
};

const safetyReport = {
  stats,
  joined_image_count: 109,
  orphan_image_count: 0,
  db_backup_count: 42,
  image_backup_count: 7,
  recent_db_backups: [
    {
      name: "clipstash.db.bak-20260608-220000",
      path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-220000",
      bytes: 61440,
      modified_at: "2026-06-08 22:00:00",
    },
  ],
  recent_image_backups: [
    {
      name: "images.bak-20260608-220001",
      path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images.bak-20260608-220001",
      bytes: 0,
      modified_at: "2026-06-08 22:00:01",
    },
  ],
};

describe("legacy safety panel", () => {
  beforeEach(() => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        return Promise.resolve({
          view: "normal",
          sort: "newest",
          offset: 0,
          limit: 30,
          total_count: 0,
          has_more: false,
          messages: [],
        });
      }
      if (command === "get_legacy_safety_report") return Promise.resolve(safetyReport);
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    invokeMock.mockReset();
    openPathMock.mockReset();
  });

  it("shows a baseline warning and backup summary when safety audit differs", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByText("数据安全"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_legacy_safety_report");
    });

    const panel = await screen.findByRole("region", { name: "数据安全审计结果" });
    expect(within(panel).getByText("旧库可能被其他版本改动过")).toBeTruthy();
    expect(within(panel).getByText("普通 11 -> 6，归档 103 -> 109，总数 114 -> 115，图片关联 107 -> 109")).toBeTruthy();
    expect(within(panel).getByText("DB备份")).toBeTruthy();
    expect(within(panel).getByText("图备份")).toBeTruthy();
    expect(within(panel).getByText("clipstash.db.bak-20260608-220000")).toBeTruthy();
    expect(within(panel).getByText("images.bak-20260608-220001")).toBeTruthy();
  });
});
