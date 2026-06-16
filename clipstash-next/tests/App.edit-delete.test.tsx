import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";

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
  normal_count: 1,
  archived_count: 0,
  total_count: 1,
};

const message = {
  id: 10,
  text_content: "旧文字",
  created_at: "2026-06-08 17:10:00",
  archived: false,
  archived_at: null,
  images: [
    {
      id: 20,
      filename: "old.png",
      path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\old.png",
      exists: true,
    },
  ],
};

const page = {
  view: "normal",
  sort: "newest",
  offset: 0,
  limit: 30,
  total_count: 1,
  has_more: false,
  messages: [message],
};

let listedMessages = [message];
let failNextTextCopy = false;
let failNextImportStage = false;
let failNextImportQueuePreview = false;
let failNextImportQueueCopy = false;
let failNextImportQueueItemPaste = false;
let failNextImportQueuePaste = false;
let failNextImportQueueArchivePaste = false;
let failNextTargetWindowRefresh = false;
let failNextTargetWindowValidation = false;
let failNextUpdate = false;
let failNextDelete = false;

const updateResult = {
  backup: {
    source_path: stats.db_path,
    backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-171000",
    bytes_copied: 61440,
  },
  audit: {
    operation: "update_message_text",
    message_id: 10,
    db_backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-171000",
    image_backup_dir: null,
  },
  message: {
    ...message,
    text_content: "新文字",
  },
};

const replaceResult = {
  backup: updateResult.backup,
  audit: {
    operation: "replace_message_images",
    message_id: 10,
    db_backup_path: updateResult.backup.backup_path,
    image_backup_dir:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images.bak-20260608-171000",
  },
  image_backup: {
    backup_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images.bak-20260608-171000",
    filenames: ["old.png"],
  },
  message: {
    ...message,
    images: [
      {
        id: 21,
        filename: "new.png",
        path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\new.png",
        exists: true,
      },
    ],
  },
};

const deleteResult = {
  backup: updateResult.backup,
  audit: {
    operation: "delete_message",
    message_id: 10,
    db_backup_path: updateResult.backup.backup_path,
    image_backup_dir:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images.bak-20260608-171000",
  },
  image_backup: replaceResult.image_backup,
  message,
};

const archiveResult = {
  backup: updateResult.backup,
  audit: {
    operation: "set_message_archived",
    message_id: 10,
    db_backup_path: updateResult.backup.backup_path,
    image_backup_dir: null,
  },
  message: {
    ...message,
    archived: true,
    archived_at: "2026-06-08 17:30:00",
  },
};

const restoreResult = {
  backup: updateResult.backup,
  audit: {
    operation: "set_message_archived",
    message_id: 10,
    db_backup_path: updateResult.backup.backup_path,
    image_backup_dir: null,
  },
  message: {
    ...message,
    archived: false,
    archived_at: null,
  },
};

const importStageResult = {
  message_id: 10,
  staged_kind: "text",
  text_length: 3,
  image_count: 1,
  first_image_filename: "old.png",
  copied_image: null,
};

const importQueuePreview = {
  message_id: 10,
  item_count: 2,
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
    {
      kind: "image",
      text: null,
      text_length: 0,
      image: message.images[0],
    },
  ],
};

const importQueueCopyResult = {
  message_id: 10,
  item_index: 0,
  staged_kind: "text",
  text_length: 3,
  image_filename: null,
  copied_image: null,
};

const importQueuePasteResult = {
  message_id: 10,
  item_index: 0,
  staged_kind: "text",
  text_length: 3,
  image_filename: null,
  target: {
    hwnd: 1001,
    process_id: 2001,
    title: "记事本",
  },
  sent_ctrl_v: true,
};

const importQueuePasteAllResult = {
  message_id: 10,
  target: {
    hwnd: 1001,
    process_id: 2001,
    title: "记事本",
  },
  requested_delay_ms: 250,
  completed_count: 2,
  failed_item_index: null,
  failure: null,
  items: [
    importQueuePasteResult,
    {
      ...importQueuePasteResult,
      item_index: 1,
      staged_kind: "image",
      text_length: 0,
      image_filename: "old.png",
    },
  ],
};

const importQueuePasteArchiveResult = {
  paste: importQueuePasteAllResult,
  archive_requested: true,
  archive_result: archiveResult,
  archive_error: null,
};

const externalWindowTargets = [
  {
    hwnd: 1001,
    process_id: 2001,
    title: "记事本",
  },
  {
    hwnd: 1002,
    process_id: 2002,
    title: "浏览器输入框",
  },
];

const externalWindowValidation = {
  valid: true,
  target: externalWindowTargets[0],
};

const tinyPngBytes = [
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1,
  0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 10, 73, 68, 65,
  84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10, 45, 180, 0, 0, 0, 0, 73,
  69, 78, 68, 174, 66, 96, 130,
];

const defaultAppSettings = {
  always_on_top: false,
  close_to_tray: true,
  archive_after_import: false,
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

describe("edit and delete guarded actions", () => {
  let appSettings = { ...defaultAppSettings };

  beforeEach(() => {
    appSettings = { ...defaultAppSettings };
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    listedMessages = [message];
    failNextTextCopy = false;
    failNextImportStage = false;
    failNextImportQueuePreview = false;
    failNextImportQueueCopy = false;
    failNextImportQueueItemPaste = false;
    failNextImportQueuePaste = false;
    failNextImportQueueArchivePaste = false;
    failNextTargetWindowRefresh = false;
    failNextTargetWindowValidation = false;
    failNextUpdate = false;
    failNextDelete = false;
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "update_app_settings") {
        appSettings = { ...appSettings, ...(args?.patch as Record<string, unknown>) };
        return Promise.resolve(appSettings);
      }
      if (command === "get_global_shortcut_errors") return Promise.resolve([]);
      if (command === "get_launch_on_startup") return Promise.resolve(false);
      if (command === "set_launch_on_startup") return Promise.resolve(Boolean(args?.enabled));
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        const offset = Number(args?.offset ?? 0);
        const limit = Number(args?.limit ?? 30);
        const messages = listedMessages.slice(offset, offset + limit);
        return Promise.resolve({
          ...page,
          total_count: listedMessages.length,
          has_more: offset + messages.length < listedMessages.length,
          offset,
          limit,
          messages,
        });
      }
      if (command === "update_legacy_message_text") {
        if (failNextUpdate) {
          failNextUpdate = false;
          return Promise.reject(new Error("更新写库失败"));
        }
        return Promise.resolve(updateResult);
      }
      if (command === "replace_legacy_message_images") return Promise.resolve(replaceResult);
      if (command === "read_legacy_image_bytes") return Promise.resolve(tinyPngBytes);
      if (command === "delete_legacy_message") {
        if (failNextDelete) {
          failNextDelete = false;
          return Promise.reject(new Error("删除写库失败"));
        }
        return Promise.resolve(deleteResult);
      }
      if (command === "set_legacy_message_archived") {
        return Promise.resolve(args?.archived ? archiveResult : restoreResult);
      }
      if (command === "copy_legacy_message_text_to_clipboard") {
        if (failNextTextCopy) {
          failNextTextCopy = false;
          return Promise.reject(new Error("文字剪贴板写入失败"));
        }
        return Promise.resolve({ message_id: message.id, text_length: 3 });
      }
      if (command === "stage_legacy_message_import_to_clipboard") {
        if (failNextImportStage) {
          failNextImportStage = false;
          return Promise.reject(new Error("导入剪贴板准备失败"));
        }
        return Promise.resolve(importStageResult);
      }
      if (command === "preview_legacy_message_import_queue") {
        if (failNextImportQueuePreview) {
          failNextImportQueuePreview = false;
          return Promise.reject(new Error("导入队列读取失败"));
        }
        return Promise.resolve(importQueuePreview);
      }
      if (command === "copy_legacy_message_import_queue_item_to_clipboard") {
        if (failNextImportQueueCopy) {
          failNextImportQueueCopy = false;
          return Promise.reject(new Error("导入队列项复制失败"));
        }
        return Promise.resolve(importQueueCopyResult);
      }
      if (command === "paste_legacy_import_queue_item") {
        if (failNextImportQueueItemPaste) {
          failNextImportQueueItemPaste = false;
          return Promise.reject(new Error("导入队列项粘贴失败"));
        }
        return Promise.resolve(importQueuePasteResult);
      }
      if (command === "paste_legacy_import_queue") {
        if (failNextImportQueuePaste) {
          failNextImportQueuePaste = false;
          return Promise.reject(new Error("导入队列整队列粘贴失败"));
        }
        return Promise.resolve(importQueuePasteAllResult);
      }
      if (command === "paste_legacy_import_queue_with_optional_archive") {
        if (failNextImportQueueArchivePaste) {
          failNextImportQueueArchivePaste = false;
          return Promise.reject(new Error("导入队列粘贴并归档失败"));
        }
        return Promise.resolve(importQueuePasteArchiveResult);
      }
      if (command === "paste_legacy_import_queue_to_recent_window") {
        if (args?.archiveAfterSuccess) {
          if (failNextImportQueueArchivePaste) {
            failNextImportQueueArchivePaste = false;
            return Promise.reject(new Error("导入队列粘贴并归档失败"));
          }
          return Promise.resolve(importQueuePasteArchiveResult);
        }
        if (failNextImportQueuePaste) {
          failNextImportQueuePaste = false;
          return Promise.reject(new Error("导入队列整队列粘贴失败"));
        }
        return Promise.resolve({
          paste: importQueuePasteAllResult,
          archive_requested: false,
          archive_result: null,
          archive_error: null,
        });
      }
      if (command === "list_external_window_targets") {
        if (failNextTargetWindowRefresh) {
          failNextTargetWindowRefresh = false;
          return Promise.reject(new Error("目标窗口刷新失败"));
        }
        return Promise.resolve(externalWindowTargets);
      }
      if (command === "validate_external_window_target") {
        if (failNextTargetWindowValidation) {
          failNextTargetWindowValidation = false;
          return Promise.reject(new Error("目标窗口校验失败"));
        }
        return Promise.resolve(externalWindowValidation);
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
    localStorage.clear();
    invokeMock.mockReset();
    isAlwaysOnTopMock.mockReset();
    previewWindowCloseMock.mockReset();
    previewWindowMock.mockReset();
    setAlwaysOnTopMock.mockReset();
    vi.restoreAllMocks();
  });

  it("toggles the current window always-on-top state", async () => {
    const user = userEvent.setup();
    render(<App />);

    const topmostButton = await screen.findByRole("button", { name: "置顶" });
    await user.click(topmostButton);

    await waitFor(() => {
      expect(setAlwaysOnTopMock).toHaveBeenCalledWith(true);
    });
    expect(screen.getByRole("button", { name: "已置顶" })).toBeTruthy();
  });

  it("loads more messages automatically near the list bottom", async () => {
    listedMessages = Array.from({ length: 31 }, (_, index) => ({
      ...message,
      id: index + 1,
      text_content: `消息 ${index + 1}`,
    }));
    render(<App />);

    expect(await screen.findByText("#1")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "加载更多" })).toBeNull();
    const list = screen.getByRole("region", { name: "消息列表" });
    Object.defineProperty(list, "clientHeight", { configurable: true, value: 300 });
    Object.defineProperty(list, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(list, "scrollTop", { configurable: true, value: 560 });
    list.dispatchEvent(new Event("scroll", { bubbles: true }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
        view: "normal",
        sort: "newest",
        offset: 30,
        limit: 30,
      });
    });
    expect(await screen.findByText("#31")).toBeTruthy();
  });

  it("updates message text after content changes", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }));

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    expect(
      Array.from(dialog.querySelectorAll(".edit-dialog-actions :is(label, button)")).map(
        (element) => element.textContent,
      ),
    ).toEqual(["选择图片", "关闭", "保存"]);
    const save = within(dialog).getByRole("button", { name: "保存" });
    expect((save as HTMLButtonElement).disabled).toBe(true);

    await user.clear(within(dialog).getByLabelText("消息内容"));
    await user.type(within(dialog).getByLabelText("消息内容"), " 新文字 ");
    expect((save as HTMLButtonElement).disabled).toBe(false);
    await user.click(save);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_legacy_message_text", {
        messageId: 10,
        textContent: "新文字",
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "replace_legacy_message_images",
      expect.anything(),
    );
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑消息 10" })).toBeNull();
    });
    expect(screen.queryByText("已保存 #10")).toBeNull();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("does not allow clearing the only content from a text-only message", async () => {
    listedMessages = [
      {
        ...message,
        id: 11,
        text_content: "只有文字",
        images: [],
      },
    ];
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#11");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 11" });
    await user.clear(within(dialog).getByLabelText("消息内容"));

    expect(
      (within(dialog).getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith(
      "update_legacy_message_text",
      expect.anything(),
    );
  });

  it("keeps edit dialog input when saving fails", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    await user.clear(within(dialog).getByLabelText("消息内容"));
    await user.type(within(dialog).getByLabelText("消息内容"), "失败后保留");

    failNextUpdate = true;
    await user.click(within(dialog).getByRole("button", { name: "保存" }));

    expect(await within(dialog).findByText("保存失败")).toBeTruthy();
    expect(within(dialog).getByText("更新写库失败")).toBeTruthy();
    expect(
      within(dialog).getByText("保存失败").closest(".operation-feedback-error"),
    ).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "编辑消息 10" })).toBeTruthy();
    expect((within(dialog).getByLabelText("消息内容") as HTMLTextAreaElement).value).toBe(
      "失败后保留",
    );
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("persists the edited text area height after manual resize", async () => {
    appSettings = { ...defaultAppSettings, edit_textarea_height: 420 };
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    const textarea = within(dialog).getByLabelText("消息内容") as HTMLTextAreaElement;
    expect(textarea.style.height).toBe("420px");

    textarea.style.height = "512px";
    fireEvent.mouseUp(textarea);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
        patch: { edit_textarea_height: 512 },
      });
    });
  });

  it("opens edit dialog when double clicking a message card by default", async () => {
    const user = userEvent.setup();
    render(<App />);

    const textButton = await screen.findByRole("button", { name: "旧文字" });
    await user.dblClick(textButton);

    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_message_text_to_clipboard", {
      messageId: 10,
    });
    expect(await screen.findByRole("dialog", { name: "编辑消息 10" })).toBeTruthy();
  });

  it("opens edit dialog when double clicking a message image", async () => {
    const user = userEvent.setup();
    render(<App />);

    const image = await screen.findByRole("img", { name: "old.png" });
    await user.dblClick(image);

    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "old.png",
    });
    expect(await screen.findByRole("dialog", { name: "编辑消息 10" })).toBeTruthy();
  });

  it("can switch message double click action to create", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const settingsDialog = await screen.findByRole("dialog", { name: "设置" });
    await user.click(within(settingsDialog).getByRole("button", { name: "新建" }));
    await user.click(within(settingsDialog).getByRole("button", { name: "关闭设置" }));

    const textButton = await screen.findByRole("button", { name: "旧文字" });
    await user.dblClick(textButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
        patch: { message_double_click_action: "create" },
      });
    });
    expect(await screen.findByRole("dialog", { name: "编辑新消息" })).toBeTruthy();
  });

  it("can switch message double click action to none", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const settingsDialog = await screen.findByRole("dialog", { name: "设置" });
    await user.click(within(settingsDialog).getByRole("button", { name: "无效果" }));
    await user.click(within(settingsDialog).getByRole("button", { name: "关闭设置" }));

    const textButton = await screen.findByRole("button", { name: "旧文字" });
    await user.dblClick(textButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
        patch: { message_double_click_action: "none" },
      });
    });
    expect(screen.queryByRole("dialog", { name: "编辑消息 10" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
  });

  it("shows existing message images in the same composer image grid", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    const imageGrid = within(dialog).getByLabelText("已选图片");
    expect(within(imageGrid).getByRole("img", { name: "old.png" })).toBeTruthy();
    expect(within(imageGrid).getByText("old.png")).toBeTruthy();
    expect(within(dialog).queryByText("已有图片")).toBeNull();
  });

  it("keeps existing edit images in the same grid when files are selected", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }));

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    await user.upload(
      within(dialog).getByLabelText("选择图片"),
      new File([new Uint8Array([9, 8])], "new.png", { type: "image/png" }),
    );
    const imageGrid = within(dialog).getByLabelText("已选图片");
    expect(within(imageGrid).getByText("old.png")).toBeTruthy();
    expect(within(imageGrid).getByText("new.png")).toBeTruthy();
    expect(within(dialog).queryByText("待替换图片")).toBeNull();
    expect(within(dialog).queryByText("保存后将被新选择的图片替换")).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("read_legacy_image_bytes", {
        filename: "old.png",
      });
      expect(invokeMock).toHaveBeenCalledWith("replace_legacy_message_images", {
        messageId: 10,
        imagesData: [tinyPngBytes, [9, 8]],
      });
    });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑消息 10" })).toBeNull();
    });
    expect(screen.queryByText("已保存 #10")).toBeNull();
  });

  it("removes a selected replacement image before saving", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }));

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    await user.upload(within(dialog).getByLabelText("选择图片"), [
      new File([new Uint8Array([1, 2])], "remove.png", { type: "image/png" }),
      new File([new Uint8Array([3, 4])], "keep.png", { type: "image/png" }),
    ]);
    await user.click(within(dialog).getByRole("button", { name: "删除图片 old.png" }));
    await user.click(within(dialog).getByRole("button", { name: "删除图片 remove.png" }));
    await user.click(within(dialog).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("replace_legacy_message_images", {
        messageId: 10,
        imagesData: [[3, 4]],
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith("replace_legacy_message_images", {
      messageId: 10,
      imagesData: [[1, 2], [3, 4]],
    });
  });

  it("deletes a message only after explicit confirmation", async () => {
    listedMessages = [{ ...message, archived: true, archived_at: "2026-06-08 17:30:00" }];
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "已归档" }));
    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "删除" }));

    const dialog = await screen.findByRole("dialog", { name: "删除消息 10" });
    const submit = within(dialog).getByRole("button", { name: "删除" });
    expect((submit as HTMLButtonElement).disabled).toBe(true);

    await user.click(within(dialog).getByLabelText("确认删除这条消息。"));
    await user.click(submit);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_legacy_message", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已删除 #10")).toBeTruthy();
    expect(screen.getByText("消息已移除。")).toBeTruthy();
  });

  it("keeps delete dialog confirmation when deleting fails", async () => {
    listedMessages = [{ ...message, archived: true, archived_at: "2026-06-08 17:30:00" }];
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "已归档" }));
    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "删除" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "删除消息 10" });
    const confirm = within(dialog).getByLabelText("确认删除这条消息。") as HTMLInputElement;
    await user.click(confirm);

    failNextDelete = true;
    await user.click(within(dialog).getByRole("button", { name: "删除" }));

    expect(await within(dialog).findByText("删除失败")).toBeTruthy();
    expect(within(dialog).getByText("删除写库失败")).toBeTruthy();
    expect(
      within(dialog).getByText("删除失败").closest(".operation-feedback-error"),
    ).toBeTruthy();
    expect(screen.getByRole("dialog", { name: "删除消息 10" })).toBeTruthy();
    expect(confirm.checked).toBe(true);
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("archives a normal message and refreshes legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    const list = screen.getByRole("region", { name: "消息列表" });
    list.scrollTop = 84;
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "归档" }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_legacy_message_archived", {
        messageId: 10,
        archived: true,
      });
    });
    expect(await screen.findByText("已归档 #10")).toBeTruthy();
    expect(screen.getByText("消息已移入归档。")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal",
      sort: "newest",
      offset: 0,
      limit: 30,
    });
    expect(screen.getByRole("region", { name: "消息列表" }).scrollTop).toBe(84);
  });

  it("restores an archived message and refreshes legacy data", async () => {
    listedMessages = [
      {
        ...message,
        archived: true,
        archived_at: "2026-06-08 17:30:00",
      },
    ];
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "已归档" }));
    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "恢复" }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_legacy_message_archived", {
        messageId: 10,
        archived: false,
      });
    });
    expect(await screen.findByText("已恢复 #10")).toBeTruthy();
    expect(screen.getByText("消息已恢复到普通列表。")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(3);
    expect(commandCallCount("list_legacy_messages")).toBe(3);
  });

  it("copies message text without invoking a write command", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "旧文字",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_legacy_message_text_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已复制 #10")).toBeTruthy();
    expect(screen.getByText("3 个字符")).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "update_legacy_message_text",
      expect.anything(),
    );
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_legacy_message_archived",
      expect.anything(),
    );
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("dismisses copy feedback when clicked", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "旧文字",
      }),
    );

    const feedback = await screen.findByText("已复制 #10");
    await user.click(feedback.closest(".operation-feedback") as HTMLElement);

    await waitFor(() => {
      expect(screen.queryByText("已复制 #10")).toBeNull();
    });
  });

  it("auto dismisses copy feedback", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "旧文字",
      }),
    );

    expect(await screen.findByText("已复制 #10")).toBeTruthy();

    await waitFor(
      () => {
        expect(screen.queryByText("已复制 #10")).toBeNull();
      },
      { timeout: 3200 },
    );
  }, 5000);

  it("shows a copy error when text clipboard writes fail", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    failNextTextCopy = true;
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "旧文字",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_legacy_message_text_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("复制失败")).toBeTruthy();
    expect(screen.getByText("文字剪贴板写入失败")).toBeTruthy();
    expect(commandCallCount("update_legacy_message_text")).toBe(0);
    expect(commandCallCount("set_legacy_message_archived")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("opens a message image preview without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const imageGrid = await screen.findByLabelText("图片缩略图");
    await user.click(await within(imageGrid).findByRole("button", { name: "old.png" }));

    expect(await screen.findByRole("tooltip", { name: "old.png" })).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "old.png",
    });
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("does not render empty text placeholder for image-only messages", async () => {
    listedMessages = [
      {
        ...message,
        text_content: null,
      },
    ];
    render(<App />);

    expect(await screen.findByRole("button", { name: "old.png" })).toBeTruthy();
    expect(screen.queryByText("无文字内容")).toBeNull();
  });

  it("expands multi-image messages, shows missing filenames, and previews images on hover", async () => {
    listedMessages = [
      {
        ...message,
        id: 12,
        images: [
          {
            id: 31,
            filename: "one.png",
            path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\one.png",
            exists: true,
          },
          {
            id: 32,
            filename: "two.png",
            path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\two.png",
            exists: true,
          },
          {
            id: 33,
            filename: "missing.png",
            path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\missing.png",
            exists: false,
          },
          {
            id: 34,
            filename: "four.png",
            path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\four.png",
            exists: true,
          },
        ],
      },
    ];
    const user = userEvent.setup();
    render(<App />);

    const imageGrid = await screen.findByLabelText("图片缩略图");
    expect(within(imageGrid).getByRole("button", { name: "one.png" })).toBeTruthy();
    expect(within(imageGrid).getByRole("button", { name: "two.png" })).toBeTruthy();
    expect(within(imageGrid).getByText("文件缺失")).toBeTruthy();
    expect(within(imageGrid).getByText("missing.png")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "four.png" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "展开 1 张图片" }));
    expect(screen.getByRole("button", { name: "four.png" })).toBeTruthy();

    await user.hover(within(imageGrid).getByRole("button", { name: "one.png" }));
    await waitFor(() => {
      expect(previewWindowMock).toHaveBeenCalledWith(
        "image-preview",
        expect.objectContaining({
          alwaysOnTop: true,
          decorations: false,
          height: expect.any(Number),
          url: expect.stringContaining("/image-preview.html"),
          width: expect.any(Number),
        }),
      );
    });
    await user.unhover(within(imageGrid).getByRole("button", { name: "one.png" }));
    await waitFor(() => {
      expect(previewWindowCloseMock).toHaveBeenCalled();
    });
  });

  it("keeps image clicks on preview even when clipboard copy would be unavailable", async () => {
    const user = userEvent.setup();
    render(<App />);

    const imageGrid = await screen.findByLabelText("图片缩略图");
    await user.click(within(imageGrid).getByRole("button", { name: "old.png" }));

    expect(await screen.findByRole("tooltip", { name: "old.png" })).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "old.png",
    });
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("stages a message import without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "准备导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stage_legacy_message_import_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已准备导入 #10")).toBeTruthy();
    expect(screen.getByText("3 个字符已进入剪贴板")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("shows an import staging error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    failNextImportStage = true;
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "准备导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stage_legacy_message_import_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("准备导入失败")).toBeTruthy();
    expect(screen.getByText("导入剪贴板准备失败")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("previews an import queue and copies a queue item without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("preview_legacy_message_import_queue", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("导入 #10")).toBeTruthy();
    expect(screen.getByText("2 项 · 文字 3 字符 · 图片 1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "复制第 1 项" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "copy_legacy_message_import_queue_item_to_clipboard",
        {
          messageId: 10,
          itemIndex: 0,
        },
      );
    });
    expect(await screen.findByText("已复制导入项 #10 / 1")).toBeTruthy();
    expect(screen.getAllByText("3 个字符已进入剪贴板").length).toBeGreaterThan(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("shows an import queue preview error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    failNextImportQueuePreview = true;
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("preview_legacy_message_import_queue", {
        messageId: 10,
      });
    });
    const alert = await screen.findByRole("alert");
    expect(within(alert).getAllByText("导入队列读取失败").length).toBeGreaterThan(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
    expect(commandCallCount("copy_legacy_message_import_queue_item_to_clipboard")).toBe(0);
  });

  it.skip("shows an import queue item copy error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    failNextImportQueueCopy = true;
    await user.click(screen.getByRole("button", { name: "复制第 1 项" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "copy_legacy_message_import_queue_item_to_clipboard",
        {
          messageId: 10,
          itemIndex: 0,
        },
      );
    });
    expect(await screen.findByText("导入队列项复制失败")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
    expect(commandCallCount("paste_legacy_import_queue_item")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
  });

  it.skip("loads, selects, and validates an external target window without pasting", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_external_window_targets");
    });

    await user.selectOptions(screen.getByLabelText("选择目标窗口"), "1001");
    expect(screen.getByText("已选择：记事本 · hwnd 1001")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "校验目标窗口" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("validate_external_window_target", {
        hwnd: 1001,
      });
    });
    expect(screen.getByText("校验通过：记事本 · pid 2001")).toBeTruthy();
    expect(commandCallCount("copy_legacy_message_import_queue_item_to_clipboard")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue_item")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("shows a target window refresh error without validating or pasting", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    failNextTargetWindowRefresh = true;
    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_external_window_targets");
    });
    expect(await screen.findByText("目标窗口刷新失败")).toBeTruthy();
    expect(commandCallCount("validate_external_window_target")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue_item")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue_with_optional_archive")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("shows a target window validation error without enabling paste actions", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));
    await user.selectOptions(screen.getByLabelText("选择目标窗口"), "1001");

    failNextTargetWindowValidation = true;
    await user.click(screen.getByRole("button", { name: "校验目标窗口" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("validate_external_window_target", {
        hwnd: 1001,
      });
    });
    expect(await screen.findByText("目标窗口校验失败")).toBeTruthy();
    expect((screen.getByRole("button", { name: "开始导入" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    expect(commandCallCount("paste_legacy_import_queue_item")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue_with_optional_archive")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("pastes a queue item only after target window validation", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    const pasteFirst = screen.getByRole("button", { name: "粘贴第 1 项" });
    expect((pasteFirst as HTMLButtonElement).disabled).toBe(true);

    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));
    await user.selectOptions(screen.getByLabelText("选择目标窗口"), "1001");
    expect((screen.getByRole("button", { name: "粘贴第 1 项" }) as HTMLButtonElement).disabled).toBe(
      true,
    );

    await user.click(screen.getByRole("button", { name: "校验目标窗口" }));
    await waitFor(() => {
      expect(screen.getByText("校验通过：记事本 · pid 2001")).toBeTruthy();
    });

    await user.click(screen.getByRole("button", { name: "粘贴第 1 项" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_item", {
        messageId: 10,
        itemIndex: 0,
        targetHwnd: 1001,
      });
    });
    expect(await screen.findByText("已粘贴导入项 #10 / 1")).toBeTruthy();
    expect(screen.getByText("3 个字符已发送到 记事本")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it.skip("shows a queue item paste error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );
    await screen.findByText("导入 #10");

    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));
    await user.selectOptions(screen.getByLabelText("选择目标窗口"), "1001");
    await user.click(screen.getByRole("button", { name: "校验目标窗口" }));
    await waitFor(() => {
      expect(screen.getByText("校验通过：记事本 · pid 2001")).toBeTruthy();
    });

    failNextImportQueueItemPaste = true;
    await user.click(screen.getByRole("button", { name: "粘贴第 1 项" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_item", {
        messageId: 10,
        itemIndex: 0,
        targetHwnd: 1001,
      });
    });
    expect(await screen.findByText("导入队列项粘贴失败")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("paste_legacy_import_queue_with_optional_archive")).toBe(0);
  });

  it("imports the whole queue to the recent external window immediately", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_to_recent_window", {
        messageId: 10,
        delayMs: 250,
        archiveAfterSuccess: false,
      });
    });
    expect(await screen.findByText("已导入 #10 · 2 项")).toBeTruthy();
    expect(screen.getByText("已发送到 记事本，间隔 250ms")).toBeTruthy();
    expect(commandCallCount("list_external_window_targets")).toBe(0);
    expect(commandCallCount("validate_external_window_target")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);

    await vi.advanceTimersByTimeAsync(2400);
    await waitFor(() => {
      expect(screen.queryByText("已导入 #10 · 2 项")).toBeNull();
    });
  });

  it("shows a whole queue paste error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    failNextImportQueuePaste = true;
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_to_recent_window", {
        messageId: 10,
        delayMs: 250,
        archiveAfterSuccess: false,
      });
    });
    expect(await screen.findByText("导入队列整队列粘贴失败")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("archives after whole queue paste when the setting is enabled", async () => {
    const user = userEvent.setup();
    appSettings = { ...appSettings, archive_after_import: true };
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "paste_legacy_import_queue_to_recent_window",
        {
          messageId: 10,
          delayMs: 250,
          archiveAfterSuccess: true,
        },
      );
    });
    expect(await screen.findByText("已导入 #10 · 2 项")).toBeTruthy();
    expect(screen.getByText("导入后已自动归档。")).toBeTruthy();
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("shows an optional archive queue paste error without refreshing legacy data", async () => {
    const user = userEvent.setup();
    failNextImportQueueArchivePaste = true;
    appSettings = { ...appSettings, archive_after_import: true };
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "paste_legacy_import_queue_to_recent_window",
        {
          messageId: 10,
          delayMs: 250,
          archiveAfterSuccess: true,
        },
      );
    });
    expect(await screen.findByText("导入队列粘贴并归档失败")).toBeTruthy();
    expect(commandCallCount("paste_legacy_import_queue")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });
});

function commandCallCount(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command).length;
}
