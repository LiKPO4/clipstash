import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { openUrl } from "@tauri-apps/plugin-opener";
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

const splittableMessage = {
  id: 10,
  text_content: "第一行\n第二行",
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

const stageResult = {
  message_id: 10,
  staged_kind: "text",
  text_length: 3,
  image_count: 1,
  first_image_filename: "old.png",
  copied_image: null,
};

const tinyPngBytes = [
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1,
  0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 10, 73, 68, 65,
  84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10, 45, 180, 0, 0, 0, 0, 73,
  69, 78, 68, 174, 66, 96, 130,
];

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
  let normalMessages = [message, neighborMessage];

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
    normalMessages = [message, neighborMessage];
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
        const messages = normalMessages.slice(offset, offset + limit);
        return Promise.resolve({
          view: "normal",
          sort: "newest",
          offset,
          limit,
          total_count: normalMessages.length,
          has_more: false,
          messages,
        });
      }
      if (command === "merge_legacy_message_with_neighbor") {
        return Promise.resolve(mergeResult);
      }
      if (command === "stage_legacy_message_import_to_clipboard") {
        return Promise.resolve(stageResult);
      }
      if (command === "read_legacy_image_bytes") {
        return Promise.resolve(new Uint8Array(tinyPngBytes));
      }
      if (command === "split_legacy_message") {
        return Promise.resolve({
          original_message_id: args?.messageId,
          messages: [
            { ...message, id: 21, text_content: "第一行", images: [image] },
            { ...message, id: 22, text_content: "第二行", images: [] },
          ],
        });
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

  it("copies the whole message via clipboard staging", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "复制" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stage_legacy_message_import_to_clipboard", {
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

  it("splits the message directly without opening the editor", async () => {
    normalMessages = [splittableMessage, neighborMessage];
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    await user.click(within(menu).getByRole("menuitem", { name: "拆分" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("split_legacy_message", {
        messageId: 10,
        textContent: "第一行\n第二行",
        imagesData: [expect.any(Array)],
      });
    });
    expect(await screen.findByText("已拆分 #10 为 2 条")).toBeTruthy();
    expect(screen.queryByRole("dialog", { name: "编辑消息 10" })).toBeNull();
  });

  it("disables split for messages without two non-empty lines", async () => {
    render(<App />);
    await screen.findByText("#10");

    const menu = await openMenu(10);
    const splitItem = within(menu).getByRole("menuitem", { name: "拆分" });

    expect((splitItem as HTMLButtonElement).disabled).toBe(true);
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
        const messages = normalMessages.slice(offset, offset + limit);
        return Promise.resolve({
          view: "normal",
          sort: "newest",
          offset,
          limit,
          total_count: normalMessages.length,
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

describe("message editor context menu", () => {
  const editorMessage = { ...message, text_content: "旧文字" };
  let clipboardText = "剪贴板内容";

  beforeEach(() => {
    clipboardText = "剪贴板内容";
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    vi.mocked(openUrl).mockClear();
    invokeMock.mockReset();
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve({ ...defaultAppSettings });
      if (command === "get_global_shortcut_errors") return Promise.resolve([]);
      if (command === "get_launch_on_startup") return Promise.resolve(false);
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        return Promise.resolve({
          view: args?.view ?? "normal",
          sort: "newest",
          offset: Number(args?.offset ?? 0),
          limit: Number(args?.limit ?? 30),
          total_count: 1,
          has_more: false,
          messages: [editorMessage],
        });
      }
      if (command === "read_legacy_image_bytes") {
        return Promise.resolve(new Uint8Array(tinyPngBytes));
      }
      if (command === "copy_text_to_clipboard") {
        return Promise.resolve(String(args?.text ?? "").length);
      }
      if (command === "read_current_clipboard") {
        return Promise.resolve(
          clipboardText.length > 0
            ? { kind: "text", text: clipboardText, image_data: null }
            : { kind: "image", text: null, image_data: [] },
        );
      }
      if (command === "split_legacy_message_selection") {
        return Promise.resolve({
          original_message_id: args?.messageId,
          message: { ...editorMessage, text_content: args?.remainingText },
          new_message: { ...editorMessage, id: 31, text_content: args?.selectedText, images: [] },
        });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
  });

  async function openEditor(user: ReturnType<typeof userEvent.setup>) {
    render(<App />);
    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );
    return await screen.findByRole("dialog", { name: "编辑消息 10" });
  }

  function openEditorMenu(dialog: HTMLElement, start: number, end: number) {
    const textarea = within(dialog).getByLabelText("消息内容") as HTMLTextAreaElement;
    textarea.focus();
    textarea.setSelectionRange(start, end);
    fireEvent.contextMenu(textarea, { clientX: 40, clientY: 60 });
    return textarea;
  }

  it("lists copy, cut, paste, search and split with selection-only items disabled", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 3, 3);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });

    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "复制",
      "剪切",
      "粘贴",
      "搜索",
      "拆分",
    ]);
    for (const label of ["复制", "剪切", "搜索", "拆分"]) {
      const item = within(menu).getByRole("menuitem", { name: label }) as HTMLButtonElement;
      expect(item.disabled).toBe(true);
    }
    expect(
      (within(menu).getByRole("menuitem", { name: "粘贴" }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });

  it("copies the selected text to the system clipboard", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 0, 2);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "复制" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_text_to_clipboard", { text: "旧文" });
    });
  });

  it("cuts the selected text out of the draft", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 1, 3);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "剪切" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_text_to_clipboard", { text: "文字" });
    });
    await waitFor(() => {
      expect((within(dialog).getByLabelText("消息内容") as HTMLTextAreaElement).value).toBe("旧");
    });
  });

  it("pastes clipboard text at the selection", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 1, 2);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "粘贴" }));

    await waitFor(() => {
      expect((within(dialog).getByLabelText("消息内容") as HTMLTextAreaElement).value).toBe(
        "旧剪贴板内容字",
      );
    });
  });

  it("reports when the clipboard holds no text", async () => {
    clipboardText = "";
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 0, 1);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "粘贴" }));

    expect(await screen.findByText("剪贴板里没有可粘贴的文字")).toBeTruthy();
  });

  it("opens a Bing search for the selected text", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 0, 3);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "搜索" }));

    await waitFor(() => {
      expect(vi.mocked(openUrl)).toHaveBeenCalledWith(
        "https://www.bing.com/search?q=%E6%97%A7%E6%96%87%E5%AD%97",
      );
    });
  });

  it("splits the selected text into a new message and closes the editor", async () => {
    const user = userEvent.setup();
    const dialog = await openEditor(user);

    openEditorMenu(dialog, 0, 2);
    const menu = await screen.findByRole("menu", { name: "编辑快捷操作" });
    await user.click(within(menu).getByRole("menuitem", { name: "拆分" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("split_legacy_message_selection", {
        messageId: 10,
        selectedText: "旧文",
        remainingText: "字",
      });
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑消息 10" })).toBeNull();
    });
    expect(await screen.findByText("已把选中文字拆成新消息 #31")).toBeTruthy();
  });
});
