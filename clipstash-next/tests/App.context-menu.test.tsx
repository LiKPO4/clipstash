import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";
import { installImageUrlMocks } from "./imageUrlMocks";

installImageUrlMocks();

const { invokeMock, isAlwaysOnTopMock, previewWindowCloseMock, previewWindowMock, setAlwaysOnTopMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
  previewWindowCloseMock: vi.fn(),
  previewWindowMock: vi.fn(),
  setAlwaysOnTopMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isAlwaysOnTop: isAlwaysOnTopMock,
    setAlwaysOnTop: setAlwaysOnTopMock,
  }),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static getByLabel = vi.fn().mockResolvedValue(null);
    close = previewWindowCloseMock.mockResolvedValue(undefined);
    once = vi.fn().mockResolvedValue(vi.fn());
    constructor(label: string, options: unknown) {
      previewWindowMock(label, options);
    }
  },
}));

vi.mock("../src/desktopPreview", () => ({
  showDesktopPreview: (...args: unknown[]) => { previewWindowMock(...args); return Promise.resolve(); },
  hideDesktopPreview: () => { previewWindowCloseMock(); return Promise.resolve(); },
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(),
  openUrl: vi.fn(),
}));

const stats = {
  data_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash",
  db_path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db",
  images_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images",
  db_exists: true,
  images_dir_exists: true,
  normal_count: 2,
  archived_count: 1,
  total_count: 3,
};

const image = {
  id: 20,
  filename: "old.png",
  path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\old.png",
  exists: true,
};

const message = {
  id: 10,
  text_content: "旧文字",
  created_at: "2026-06-08 17:10:00",
  archived: false,
  archived_at: null,
  images: [image],
};

const neighborMessage = {
  id: 9,
  text_content: "上一条",
  created_at: "2026-06-08 17:00:00",
  archived: false,
  archived_at: null,
  images: [],
};

const archivedMessage = {
  id: 5,
  text_content: "已归档文字",
  created_at: "2026-06-07 17:10:00",
  archived: true,
  archived_at: "2026-06-07 18:00:00",
  images: [],
};

const defaultAppSettings = {
  always_on_top: false,
  close_to_tray: true,
  launch_on_startup: false,
  main_window_state: null,
  archive_after_import: false,
  archive_after_export: false,
  match_blank_lines_to_images: false,
  message_double_click_action: "edit",
  paste_interval_ms: 250,
  show_hotkey: "Ctrl+Shift+V",
  capture_hotkey: "Ctrl+Alt+V",
  hover_delay: 0.8,
  scroll_lines: 1,
  font_scale: 0,
  edit_textarea_height: 360,
  sort: "newest",
};

const mergeResult = {
  merged_message_id: 10,
  removed_message_id: 9,
  message: {
    ...message,
    text_content: "旧文字\n上一条",
  },
};

const importQueuePreview = {
  message_id: 10,
  item_count: 1,
  text_length: 3,
  image_count: 1,
  skipped_missing_image_count: 0,
  items: [
    {
      kind: "text",
      text: "旧文字",
      text_length: 3,
      image: null,
    },
  ],
};

describe("message card context menu", () => {
  let appSettings = { ...defaultAppSettings };

  function cardOf(id: number) {
    const title = screen.getByText(`#${id}`);
    return title.closest("article") as HTMLElement;
  }

  async function openMenu(id: number) {
    fireEvent.contextMenu(cardOf(id));
    return await screen.findByRole("menu", { name: "消息快捷操作" });
  }

  beforeEach(() => {
    appSettings = { ...defaultAppSettings };
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "get_global_shortcut_errors") return Promise.resolve([]);
      if (command === "get_launch_on_startup") return Promise.resolve(false);
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        const offset = Number(args?.offset ?? 0);
        const limit = Number(args?.limit ?? 30);
        if (args?.view === "archived") {
          return Promise.resolve({
            view: "archived",
            sort: "newest",
            offset,
            limit,
            total_count: 1,
            has_more: false,
            messages: [archivedMessage],
          });
        }
        const messages = [message, neighborMessage].slice(offset, offset + limit);
        return Promise.resolve({
          view: "normal",
          sort: "newest",
          offset,
          limit,
          total_count: 2,
          has_more: false,
          messages,
        });
      }
      if (command === "merge_legacy_message_with_neighbor") {
        return Promise.resolve(mergeResult);
      }
      if (command === "copy_legacy_message_text_to_clipboard") {
        return Promise.resolve({ message_id: message.id, text_length: 3 });
      }
      if (command === "preview_legacy_message_import_queue") {
        return Promise.resolve(importQueuePreview);
      }
      if (command === "paste_legacy_import_queue_to_recent_window") {
        return Promise.resolve({
          paste: {
            message_id: 10,
            target: { hwnd: 1001, process_id: 2001, title: "记事本" },
            requested_delay_ms: 250,
            completed_count: 1,
            failed_item_index: null,
            failure: null,
            items: [],
          },
          archive_requested: false,
          archive_result: null,
          archive_error: null,
        });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
  });

  it("shows copy, paste, split, and merge actions for normal messages", async () => {
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);

    const labels = within(menu)
      .getAllByRole("menuitem")
      .map((item) => item.textContent);
    expect(labels).toEqual(["复制", "粘贴", "拆分", "向下合并", "向上合并"]);
  });

  it("copies the message text from the context menu", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "复制" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_legacy_message_text_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已复制 #10")).toBeTruthy();
  });

  it("opens the import paste queue from the context menu", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "粘贴" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("preview_legacy_message_import_queue", {
        messageId: 10,
        matchBlankLinesToImages: false,
      });
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_to_recent_window", {
        messageId: 10,
        delayMs: 250,
        archiveAfterSuccess: false,
        matchBlankLinesToImages: false,
      });
    });
  });

  it("opens the editor for splitting from the context menu", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "拆分" }));

    expect(await screen.findByRole("dialog", { name: "编辑消息 10" })).toBeTruthy();
  });

  it("merges the message into the next one downward", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "向下合并" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("merge_legacy_message_with_neighbor", {
        messageId: 10,
        direction: "down",
        view: "normal",
        sort: "newest",
      });
    });
    expect(await screen.findByText("已合并 #10 与 #9")).toBeTruthy();
  });

  it("merges the message into the previous one upward", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "向上合并" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("merge_legacy_message_with_neighbor", {
        messageId: 10,
        direction: "up",
        view: "normal",
        sort: "newest",
      });
    });
  });

  it("shows only copy and merge actions for archived messages", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");
    await user.click(await screen.findByRole("button", { name: "已归档" }));
    await screen.findByText("#5");

    const menu = await openMenu(5);

    const labels = within(menu)
      .getAllByRole("menuitem")
      .map((item) => item.textContent);
    expect(labels).toEqual(["复制", "向下合并", "向上合并"]);
  });

  it("closes the context menu on Escape", async () => {
    render(<App />);
    await screen.findByText("#10");

    await openMenu(10);
    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => {
      expect(screen.queryByRole("menu", { name: "消息快捷操作" })).toBeNull();
    });
  });

  it("reports merge failures in the feedback area", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "get_global_shortcut_errors") return Promise.resolve([]);
      if (command === "get_launch_on_startup") return Promise.resolve(false);
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        const offset = Number(args?.offset ?? 0);
        const limit = Number(args?.limit ?? 30);
        const messages = [message, neighborMessage].slice(offset, offset + limit);
        return Promise.resolve({
          view: "normal",
          sort: "newest",
          offset,
          limit,
          total_count: 2,
          has_more: false,
          messages,
        });
      }
      if (command === "merge_legacy_message_with_neighbor") {
        return Promise.reject(new Error("下方没有相邻消息"));
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "向下合并" }));

    expect(await screen.findByText("合并失败")).toBeTruthy();
    expect(await screen.findByText("下方没有相邻消息")).toBeTruthy();
  });
});
