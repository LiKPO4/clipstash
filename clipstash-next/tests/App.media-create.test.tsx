import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";

const { invokeMock, isAlwaysOnTopMock, setAlwaysOnTopMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
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
    close = vi.fn().mockResolvedValue(undefined);
    once = vi.fn().mockResolvedValue(vi.fn());
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
  normal_count: 11,
  archived_count: 103,
  total_count: 114,
};

const emptyPage = {
  view: "normal",
  sort: "newest",
  offset: 0,
  limit: 30,
  total_count: 0,
  has_more: false,
  messages: [],
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
  edit_textarea_height: 420,
  sort: "newest",
};

const createResult = {
  backup: {
    source_path: stats.db_path,
    backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-170000",
    bytes_copied: 61440,
  },
  audit: {
    operation: "create_image_message",
    message_id: 200,
    db_backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-170000",
    image_backup_dir: null,
  },
  message: {
    id: 200,
    text_content: null,
    created_at: "2026-06-08 17:00:00",
    archived: false,
    archived_at: null,
    images: [
      {
        id: 301,
        filename: "clipstash-next-test.png",
        path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\clipstash-next-test.png",
        exists: true,
      },
    ],
  },
};

describe("media create form", () => {
  let appSettings = { ...defaultAppSettings };

  beforeEach(() => {
    appSettings = { ...defaultAppSettings };
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "update_app_settings") {
        appSettings = { ...appSettings, ...(args?.patch as Record<string, unknown>) };
        return Promise.resolve(appSettings);
      }
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") return Promise.resolve(emptyPage);
      if (command === "create_legacy_image_message") return Promise.resolve(createResult);
      if (command === "create_legacy_mixed_message") {
        return Promise.resolve({
          ...createResult,
          audit: {
            ...createResult.audit,
            operation: "create_mixed_message",
          },
          message: {
            ...createResult.message,
            text_content: "配套文字",
          },
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    localStorage.clear();
    invokeMock.mockReset();
    isAlwaysOnTopMock.mockReset();
    setAlwaysOnTopMock.mockReset();
  });

  it("requires message content before saving", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByText("11 条消息")).toBeTruthy();
    expect(screen.getByText("103 条消息")).toBeTruthy();
    const panel = await openMediaCreateDialog(user);
    expect(within(panel).queryByText("写入旧库前会自动备份")).toBeNull();
    expect(within(panel).queryByRole("checkbox")).toBeNull();
    const submit = within(panel).getByRole("button", { name: "保存" });

    expect((submit as HTMLButtonElement).disabled).toBe(true);

    await user.type(within(panel).getByLabelText("消息内容"), "新文字");

    await waitFor(() => {
      expect((submit as HTMLButtonElement).disabled).toBe(false);
    });
  });

  it("uses the same roomy editor layout for new messages", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    expect(panel.classList.contains("edit-message-dialog")).toBe(true);
    expect(within(panel).queryByText("可粘贴图片或选择文件")).toBeNull();
    expect(within(panel).getByRole("button", { name: "关闭" })).toBeTruthy();
    expect(
      Array.from(panel.querySelectorAll(".edit-dialog-actions :is(label, button)")).map(
        (element) => element.textContent,
      ),
    ).toEqual(["选择图片", "关闭", "保存"]);
    expect((within(panel).getByLabelText("消息内容") as HTMLTextAreaElement).style.height).toBe(
      "420px",
    );
    await waitFor(() => {
      expect(within(panel).getByLabelText("消息内容")).toBe(document.activeElement);
    });
  });

  it("opens the composer when double clicking the empty list area", async () => {
    const user = userEvent.setup();
    render(<App />);

    const emptyArea = await screen.findByRole("button", {
      name: "当前视图没有消息。双击空白处创建。",
    });
    await user.dblClick(emptyArea);

    expect(await screen.findByRole("dialog", { name: "编辑新消息" })).toBeTruthy();
  });

  it("creates an image-only message and refreshes legacy data after success", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    await user.upload(
      within(panel).getByLabelText("选择图片"),
      new File([new Uint8Array([1, 2, 3])], "pixel.png", { type: "image/png" }),
    );
    await user.click(within(panel).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
        imagesData: [[1, 2, 3]],
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "create_legacy_mixed_message",
      expect.anything(),
    );
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
    });
    expect(screen.queryByText("已保存 #200")).toBeNull();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("creates a mixed text and image message when text is present", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    await user.type(within(panel).getByLabelText("消息内容"), " 配套文字 ");
    await user.upload(
      within(panel).getByLabelText("选择图片"),
      new File([new Uint8Array([4, 5])], "pixel.png", { type: "image/png" }),
    );
    await user.click(within(panel).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_mixed_message", {
        textContent: "配套文字",
        imagesData: [[4, 5]],
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "create_legacy_image_message",
      expect.anything(),
    );
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
    });
    expect(screen.queryByText("已保存 #200")).toBeNull();
  });

  it("accepts pasted images in the same message composer", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    const textarea = within(panel).getByLabelText("消息内容");
    const file = new File([new Uint8Array([7, 8, 9])], "pasted.png", {
      type: "image/png",
    });
    firePaste(textarea, file);
    expect(within(panel).getByRole("button", { name: "删除图片 pasted.png" })).toBeTruthy();
    expect(await within(panel).findByRole("img", { name: "pasted.png" })).toBeTruthy();

    await user.click(within(panel).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
        imagesData: [[7, 8, 9]],
      });
    });
  });

  it("accepts dropped images in the same message composer", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    const file = new File([new Uint8Array([8, 6])], "dropped.png", {
      type: "image/png",
    });
    fireDrop(panel, file);

    expect(within(panel).getByRole("button", { name: "删除图片 dropped.png" })).toBeTruthy();
    expect(await within(panel).findByRole("img", { name: "dropped.png" })).toBeTruthy();

    await user.click(within(panel).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
        imagesData: [[8, 6]],
      });
    });
  });

  it("adds dropped non-image file paths to the message text", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    const textarea = within(panel).getByLabelText("消息内容") as HTMLTextAreaElement;
    const file = new File(["hello"], "notes.txt", { type: "text/plain" });
    Object.defineProperty(file, "path", {
      configurable: true,
      value: "D:\\WORKS\\notes.txt",
    });

    fireDrop(panel, file);
    expect(textarea.value).toBe("D:\\WORKS\\notes.txt");
  });

  it("closes the composer after saving without showing create feedback", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await openMediaCreateDialog(user);
    await user.upload(
      within(panel).getByLabelText("选择图片"),
      new File([new Uint8Array([1, 2, 3])], "pixel.png", { type: "image/png" }),
    );
    await user.click(within(panel).getByRole("button", { name: "保存" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
    });
    expect(screen.queryByText("已保存 #200")).toBeNull();
  });
});

function commandCallCount(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command).length;
}

async function openMediaCreateDialog(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "+ 新建" }));
  return screen.findByRole("dialog", { name: "编辑新消息" });
}

function firePaste(target: HTMLElement, file: File) {
  fireEvent.paste(target, {
    clipboardData: {
      files: [file],
    },
  });
}

function fireDrop(target: HTMLElement, file: File) {
  fireEvent.drop(target, {
    dataTransfer: {
      files: [file],
      types: ["Files"],
    },
  });
}
