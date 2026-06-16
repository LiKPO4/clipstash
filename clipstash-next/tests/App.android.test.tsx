import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { canShareMock, invokeMock, isAlwaysOnTopMock, setAlwaysOnTopMock, shareMock } = vi.hoisted(() => ({
  canShareMock: vi.fn(),
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
  setAlwaysOnTopMock: vi.fn(),
  shareMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isAlwaysOnTop: isAlwaysOnTopMock,
    onDragDropEvent: vi.fn().mockResolvedValue(vi.fn()),
    setAlwaysOnTop: setAlwaysOnTopMock,
  }),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static getByLabel = vi.fn().mockResolvedValue(null);
    close = vi.fn().mockResolvedValue(undefined);
    once = vi.fn().mockResolvedValue(vi.fn());
  },
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(),
  openUrl: vi.fn(),
}));

const stats = {
  data_dir: "/data/user/0/com.clipstash.next/files",
  db_path: "/data/user/0/com.clipstash.next/files/clipstash.db",
  images_dir: "/data/user/0/com.clipstash.next/files/images",
  db_exists: true,
  images_dir_exists: true,
  normal_count: 1,
  archived_count: 0,
  total_count: 1,
};

const defaultAppSettings = {
  always_on_top: false,
  close_to_tray: true,
  archive_after_import: false,
  paste_interval_ms: 250,
  show_hotkey: "Ctrl+Shift+V",
  capture_hotkey: "Ctrl+Alt+V",
  hover_delay: 0.8,
  scroll_lines: 1,
  font_scale: 0,
  edit_textarea_height: 360,
  sort: "newest",
};

const normalPage = {
  view: "normal",
  sort: "newest",
  offset: 0,
  limit: 30,
  total_count: 1,
  has_more: false,
  messages: [
    {
      id: 1,
      text_content: "手机记录",
      created_at: "2026-06-16 10:00:00",
      archived: false,
      archived_at: null,
      images: [],
    },
  ],
};

describe("android shell", () => {
  beforeEach(() => {
    vi.resetModules();
    Object.defineProperty(window.navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Linux; Android 15) AppleWebKit/537.36",
    });
    Object.defineProperty(window.navigator, "canShare", {
      configurable: true,
      value: canShareMock,
    });
    Object.defineProperty(window.navigator, "share", {
      configurable: true,
      value: shareMock,
    });
    canShareMock.mockReturnValue(true);
    shareMock.mockResolvedValue(undefined);
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_app_settings") return Promise.resolve(defaultAppSettings);
      if (command === "update_app_settings") return Promise.resolve(defaultAppSettings);
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") return Promise.resolve(normalPage);
      if (command === "read_legacy_image_bytes") return Promise.resolve([]);
      if (command === "export_normal_data_zip_bytes") {
        return Promise.resolve({
          filename: "clipstash-export-20260616-100000.zip",
          export: {
            path: "/tmp/clipstash-export.zip",
            message_count: 1,
            image_count: 0,
            skipped_archived_count: 0,
            skipped_missing_image_count: 0,
            skipped_empty_message_count: 0,
            bytes: 512,
          },
          bytes: [80, 75, 3, 4],
        });
      }
      if (command === "import_data_zip_bytes") {
        return Promise.resolve({
          path: "/tmp/clipstash-import.zip",
          inserted_messages: 1,
          skipped_messages: 0,
          imported_images: 0,
          stats: { ...stats, normal_count: 2, total_count: 2 },
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    localStorage.clear();
    vi.unstubAllGlobals();
    invokeMock.mockReset();
    isAlwaysOnTopMock.mockReset();
    setAlwaysOnTopMock.mockReset();
    canShareMock.mockReset();
    shareMock.mockReset();
  });

  it("uses android actions and hides desktop-only controls", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    const { container } = render(<App />);

    expect(await screen.findByRole("button", { name: "导出" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "置顶" })).toBeNull();
    expect(screen.queryByRole("button", { name: "导入" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "导出" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_normal_data_zip_bytes");
    });
    expect(shareMock).toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    const storage = within(dialog).getByRole("region", { name: "本地存储" });
    expect(within(storage).getByRole("button", { name: "导出数据" })).toBeTruthy();
    const importButton = within(storage).getByRole("button", { name: "导入数据" });
    expect(importButton).toBeTruthy();
    expect(within(storage).queryByRole("button", { name: "迁移旧数据" })).toBeNull();
    expect(within(storage).queryByRole("button", { name: "迁移数据目录" })).toBeNull();
    expect(within(storage).queryByRole("button", { name: "修复数据目录" })).toBeNull();
    expect(within(dialog).queryByText("开机自启动")).toBeNull();
    expect(within(dialog).queryByText("呼出界面快捷键")).toBeNull();
    expect(invokeMock).not.toHaveBeenCalledWith("get_launch_on_startup");
    expect(invokeMock).not.toHaveBeenCalledWith("get_global_shortcut_errors");
    expect(isAlwaysOnTopMock).not.toHaveBeenCalled();

    await user.click(importButton);
    const input = container.querySelector<HTMLInputElement>('input[type="file"][accept*=".zip"]');
    expect(input).toBeTruthy();
    fireEvent.change(input!, {
      target: {
        files: [new File([new Uint8Array([80, 75, 3, 4])], "clipstash.zip", { type: "application/zip" })],
      },
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_data_zip_bytes", {
        filename: "clipstash.zip",
        bytes: [80, 75, 3, 4],
      });
    });
  });
});
