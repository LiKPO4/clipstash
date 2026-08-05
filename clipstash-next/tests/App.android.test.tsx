import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  canShareMock,
  invokeMock,
  isAlwaysOnTopMock,
  onDragDropEventMock,
  openPathMock,
  setAlwaysOnTopMock,
  shareMock,
  webviewWindowGetByLabelMock,
} = vi.hoisted(() => ({
  canShareMock: vi.fn(),
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
  onDragDropEventMock: vi.fn(),
  openPathMock: vi.fn(),
  setAlwaysOnTopMock: vi.fn(),
  shareMock: vi.fn(),
  webviewWindowGetByLabelMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isAlwaysOnTop: isAlwaysOnTopMock,
    onDragDropEvent: onDragDropEventMock,
    setAlwaysOnTop: setAlwaysOnTopMock,
  }),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static getByLabel = webviewWindowGetByLabelMock;
    close = vi.fn().mockResolvedValue(undefined);
    once = vi.fn().mockResolvedValue(vi.fn());
  },
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: openPathMock,
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
  launch_on_startup: false,
  main_window_state: null,
  archive_after_import: false,
  archive_after_export: false,
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

const createdMessage = {
  id: 2,
  text_content: "分享文字",
  created_at: "2026-06-16 10:10:00",
  archived: false,
  archived_at: null,
  images: [],
};

const createResult = {
  backup: {
    source_path: stats.db_path,
    backup_path: `${stats.db_path}.bak-20260616-101000`,
    bytes_copied: 4096,
  },
  audit: {
    operation: "create_message",
    message_id: 2,
    db_backup_path: `${stats.db_path}.bak-20260616-101000`,
    image_backup_dir: null,
  },
  message: createdMessage,
};

describe("android shell", () => {
  let appSettings = { ...defaultAppSettings };
  let listedPage = normalPage;
  let androidCheckForUpdatesMock: ReturnType<typeof vi.fn>;
  let androidConsumePendingUpdateMock: ReturnType<typeof vi.fn>;
  let androidCopyTextMock: ReturnType<typeof vi.fn>;
  let androidDownloadAndInstallApkMock: ReturnType<typeof vi.fn>;
  let androidRefreshWidgetsMock: ReturnType<typeof vi.fn>;
  let androidShareZipMock: ReturnType<typeof vi.fn> | null = null;

  beforeEach(() => {
    vi.resetModules();
    appSettings = { ...defaultAppSettings };
    listedPage = normalPage;
    androidCheckForUpdatesMock = vi.fn().mockReturnValue(true);
    androidConsumePendingUpdateMock = vi.fn().mockReturnValue("");
    androidCopyTextMock = vi.fn().mockReturnValue("ok");
    androidDownloadAndInstallApkMock = vi.fn().mockReturnValue(true);
    androidRefreshWidgetsMock = vi.fn();
    androidShareZipMock = null;
    window.ClipStashAndroid = {
      checkForUpdates: androidCheckForUpdatesMock,
      consumePendingUpdate: androidConsumePendingUpdateMock,
      copyText: androidCopyTextMock,
      downloadAndInstallApk: androidDownloadAndInstallApkMock,
      refreshWidgets: androidRefreshWidgetsMock,
    };
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
    openPathMock.mockResolvedValue(undefined);
    shareMock.mockResolvedValue(undefined);
    isAlwaysOnTopMock.mockResolvedValue(false);
    onDragDropEventMock.mockResolvedValue(vi.fn());
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    webviewWindowGetByLabelMock.mockResolvedValue(null);
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "update_app_settings") {
        appSettings = { ...appSettings, ...(args?.patch as Record<string, unknown>) };
        return Promise.resolve(appSettings);
      }
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "get_legacy_message") {
        const messageId = args?.messageId as number;
        return Promise.resolve(normalPage.messages.find((message) => message.id === messageId));
      }
      if (command === "list_legacy_messages") return Promise.resolve(listedPage);
      if (command === "read_legacy_image_bytes") return Promise.resolve(new Uint8Array(0));
      if (command === "create_legacy_text_message") return Promise.resolve(createResult);
      if (command === "create_legacy_image_message") {
        return Promise.resolve({
          ...createResult,
          message: { ...createdMessage, text_content: null, images: [{ id: 20, filename: "share.png", path: "/images/share.png", exists: true }] },
        });
      }
      if (command === "create_legacy_mixed_message") {
        return Promise.resolve({
          ...createResult,
          message: { ...createdMessage, images: [{ id: 20, filename: "share.png", path: "/images/share.png", exists: true }] },
        });
      }
      if (command === "split_legacy_message") {
        return Promise.resolve({
          original_message_id: args?.messageId,
          messages: [
            { ...normalPage.messages[0], id: 3, text_content: "第一行" },
            { ...normalPage.messages[0], id: 4, text_content: "第二行" },
          ],
        });
      }
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
          message_ids: [1],
        });
      }
      if (command === "archive_exported_messages") {
        listedPage = { ...normalPage, total_count: 0, messages: [] };
        return Promise.resolve({ ...stats, normal_count: 0, archived_count: 1 });
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
    onDragDropEventMock.mockReset();
    setAlwaysOnTopMock.mockReset();
    canShareMock.mockReset();
    openPathMock.mockReset();
    shareMock.mockReset();
    webviewWindowGetByLabelMock.mockReset();
    Reflect.deleteProperty(window, "ClipStashAndroid");
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
    expect(within(dialog).queryByText("悬浮预览延迟")).toBeNull();
    expect(within(dialog).queryByText("滚动速度")).toBeNull();
    expect(within(dialog).queryByText("粘贴间隔")).toBeNull();
    expect(within(dialog).getByText("导出后自动归档")).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith("get_launch_on_startup");
    expect(invokeMock).not.toHaveBeenCalledWith("get_global_shortcut_errors");
    expect(isAlwaysOnTopMock).not.toHaveBeenCalled();
    expect(onDragDropEventMock).not.toHaveBeenCalled();
    expect(setAlwaysOnTopMock).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: "检查更新" }));
    expect(androidCheckForUpdatesMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).not.toHaveBeenCalledWith("fetch_latest_github_release");
    expect(invokeMock).not.toHaveBeenCalledWith("download_and_open_update_installer", expect.anything());
    act(() => {
      window.dispatchEvent(new CustomEvent("clipstash-android-update", {
        detail: {
          status: "checked",
          message: "检查完成",
          release: {
            tag_name: "v99.0.0",
            html_url: "https://github.com/LiKPO4/clipstash/releases/tag/v99.0.0",
            assets: [
              {
                name: "ClipStash.Next_99.0.0_android-universal-release-signed.apk",
                browser_download_url: "https://github.com/LiKPO4/clipstash/releases/download/v99.0.0/ClipStash.Next_99.0.0_android-universal-release-signed.apk",
              },
            ],
          },
        },
      }));
    });
    expect(await within(dialog).findByText("发现新版本 99.0.0")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "下载并安装" }));
    expect(androidDownloadAndInstallApkMock).toHaveBeenCalledWith(
      "https://github.com/LiKPO4/clipstash/releases/download/v99.0.0/ClipStash.Next_99.0.0_android-universal-release-signed.apk",
      "ClipStash.Next_99.0.0_android-universal-release-signed.apk",
    );

    await user.click(within(dialog).getByRole("button", { name: "关闭设置" }));
    await user.click(screen.getByRole("button", { name: "+ 新建" }));
    const composer = await screen.findByRole("dialog", { name: "编辑新消息" });
    expect(within(composer).getByLabelText("关闭新消息")).toBeTruthy();
    expect(composer.querySelector(".edit-dialog-actions")).toBeNull();
    expect(within(composer).getByLabelText("选择图片")).toBeTruthy();
    expect(within(composer).queryByRole("button", { name: "关闭" })).toBeNull();
    expect(within(composer).getByRole("button", { name: "保存" })).toBeTruthy();
    expect((within(composer).getByLabelText("消息内容") as HTMLTextAreaElement).style.fontSize).toBe("1.3em");

    await user.upload(
      within(composer).getByLabelText("选择图片"),
      new File([new Uint8Array([1, 2, 3])], "phone.png", { type: "image/png" }),
    );
    await within(composer).findByRole("img", { name: "phone.png" });
    const previewButton = composer.querySelector<HTMLButtonElement>(".composer-image-tile");
    expect(previewButton).toBeTruthy();
    await user.click(previewButton!);
    await user.click(await screen.findByRole("button", { name: "关闭图片预览 phone.png" }));

    await user.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => {
      expect(androidRefreshWidgetsMock).toHaveBeenCalled();
    });

    fireEvent.click(composer.parentElement!);
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
    });
    await user.click(screen.getByRole("button", { name: "设置" }));
    const reopenedDialog = await screen.findByRole("dialog", { name: "设置" });
    const reopenedStorage = within(reopenedDialog).getByRole("region", { name: "本地存储" });
    const reopenedImportButton = within(reopenedStorage).getByRole("button", { name: "导入数据" });

    await user.click(reopenedImportButton);
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

  it("opens the exported zip path when android file sharing is unavailable", async () => {
    canShareMock.mockReturnValue(false);
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "导出" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_normal_data_zip_bytes");
    });
    expect(shareMock).not.toHaveBeenCalled();
    expect(openPathMock).toHaveBeenCalledWith("/tmp/clipstash-export.zip");
    expect(await screen.findByText("数据包已导出")).toBeTruthy();
  });

  it("uses the native android zip share bridge when available", async () => {
    androidShareZipMock = vi.fn();
    window.ClipStashAndroid = {
      refreshWidgets: androidRefreshWidgetsMock,
      shareZip: androidShareZipMock,
    };
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "导出" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_normal_data_zip_bytes");
    });
    expect(androidShareZipMock).toHaveBeenCalledWith("/tmp/clipstash-export.zip");
    expect(shareMock).not.toHaveBeenCalled();
    expect(openPathMock).not.toHaveBeenCalledWith("/tmp/clipstash-export.zip");
  });

  it("archives exported messages when the android setting is enabled", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    await user.click(within(dialog).getByRole("checkbox", { name: /导出后自动归档/ }));
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { archive_after_export: true },
    });
    await user.click(within(dialog).getByRole("button", { name: "关闭设置" }));
    await user.click(screen.getByRole("button", { name: "导出" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("archive_exported_messages", {
        messageIds: [1],
      });
    });
    expect(androidRefreshWidgetsMock).toHaveBeenCalled();
    expect(await screen.findByText(/已自动归档/)).toBeTruthy();
  });

  it("starts the same export flow from the widget share action", async () => {
    const consumePendingWidgetAction = vi.fn().mockReturnValueOnce("export").mockReturnValue("");
    androidShareZipMock = vi.fn();
    window.ClipStashAndroid = {
      consumePendingWidgetAction,
      refreshWidgets: androidRefreshWidgetsMock,
      shareZip: androidShareZipMock,
    };
    const { default: App } = await import("../src/App");
    render(<App />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_normal_data_zip_bytes");
    });
    expect(androidShareZipMock).toHaveBeenCalledWith("/tmp/clipstash-export.zip");
  });

  it("opens the selected widget message in the edit dialog", async () => {
    const consumePendingWidgetAction = vi.fn().mockReturnValueOnce("edit:1").mockReturnValue("");
    window.ClipStashAndroid = { consumePendingWidgetAction };
    const { default: App } = await import("../src/App");
    render(<App />);

    expect(await screen.findByRole("dialog", { name: "编辑消息 1" })).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("get_legacy_message", { messageId: 1 });
    expect((screen.getByRole("textbox", { name: "消息内容" }) as HTMLTextAreaElement).value)
      .toBe("手机记录");
  });

  it("creates a message from android shared text", async () => {
    window.ClipStashAndroid = {
      // 队列契约：消费一次后返回空串，模拟原生桥逐条消费直到队列为空
      consumePendingShare: vi
        .fn()
        .mockReturnValueOnce(JSON.stringify({ text: "  分享文字  ", images: [] }))
        .mockReturnValue(""),
      refreshWidgets: androidRefreshWidgetsMock,
    };
    const { default: App } = await import("../src/App");
    render(<App />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_text_message", {
        textContent: "分享文字",
      });
    });
    expect(await screen.findByText("分享已保存")).toBeTruthy();
    expect(await screen.findByText("已创建 #2")).toBeTruthy();
    expect(androidRefreshWidgetsMock).toHaveBeenCalled();
  });

  it("creates a message from android shared image", async () => {
    window.ClipStashAndroid = {
      consumePendingShare: vi
        .fn()
        .mockReturnValueOnce(
          JSON.stringify({ text: "", images: [{ mimeType: "image/png", data: "AQID" }] }),
        )
        .mockReturnValue(""),
      refreshWidgets: androidRefreshWidgetsMock,
    };
    const { default: App } = await import("../src/App");
    render(<App />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
        imagesData: [[1, 2, 3]],
      });
    });
    expect(await screen.findByText("分享已保存")).toBeTruthy();
  });

  it("uses edit as the default double click action on android messages", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    const textButton = await screen.findByRole("button", { name: "手机记录" });
    await user.dblClick(textButton);

    expect(await screen.findByRole("dialog", { name: "编辑消息 1" })).toBeTruthy();
  });

  it("closes the edit dialog instead of leaving the app on android back", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    const textButton = await screen.findByRole("button", { name: "手机记录" });
    await user.dblClick(textButton);
    expect(await screen.findByRole("dialog", { name: "编辑消息 1" })).toBeTruthy();

    const backEvent = new Event("clipstash-android-back", { cancelable: true });
    window.dispatchEvent(backEvent);

    expect(backEvent.defaultPrevented).toBe(true);
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "编辑消息 1" })).toBeNull();
    });
    expect(invokeMock).not.toHaveBeenCalledWith("update_legacy_message_text", expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith("replace_legacy_message_images", expect.anything());
  });

  it("copies message text through the Android system clipboard", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "手机记录" }));

    await waitFor(() => {
      expect(androidCopyTextMock).toHaveBeenCalledWith("手机记录");
    });
    expect(await screen.findByText("4 个字符")).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_message_text_to_clipboard", {
      messageId: 1,
    });
  });

  it("does not invoke the Windows clipboard shortcut on Android", async () => {
    const { default: App } = await import("../src/App");
    render(<App />);
    await screen.findByRole("button", { name: "手机记录" });

    fireEvent.keyDown(window, { key: "v", ctrlKey: true });

    expect(invokeMock).not.toHaveBeenCalledWith("read_current_clipboard");
    expect(screen.queryByRole("dialog", { name: "编辑新消息" })).toBeNull();
  });

  it("places split before image and save in the Android edit header", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    const card = (await screen.findByText("#1")).closest("article") as HTMLElement;
    await user.click(within(card).getByRole("button", { name: "编辑" }));
    const dialog = await screen.findByRole("dialog", { name: "编辑消息 1" });
    const actions = dialog.querySelector(".composer-mobile-actions");
    expect(actions).toBeTruthy();
    expect(Array.from(actions!.querySelectorAll(":scope > :is(button, label)")).map((item) => item.textContent)).toEqual([
      "拆分",
      "图片",
      "保存",
      "×",
    ]);

    const textarea = within(dialog).getByLabelText("消息内容");
    await user.clear(textarea);
    await user.type(textarea, "第一行{enter}第二行");
    await user.click(within(dialog).getByRole("button", { name: "拆分" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("split_legacy_message", {
        messageId: 1,
        textContent: "第一行\n第二行",
        imagesData: [],
      });
    });
  });

  it("consumes a pending native update result when the WebView event is missed", async () => {
    androidConsumePendingUpdateMock
      .mockReturnValueOnce("")
      .mockReturnValueOnce(JSON.stringify({
        status: "checked",
        message: "检查完成",
        release: {
          tag_name: "v99.0.0",
          html_url: "https://github.com/LiKPO4/clipstash/releases/tag/v99.0.0",
          assets: [],
        },
      }));
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    await user.click(within(dialog).getByRole("button", { name: "检查更新" }));

    expect(await within(dialog).findByText("发现新版本 99.0.0")).toBeTruthy();
    expect((within(dialog).getByRole("button", { name: "检查更新" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("opens message image previews instead of copying images on android", async () => {
    listedPage = {
      ...normalPage,
      messages: [
        {
          ...normalPage.messages[0],
          images: [
            {
              id: 10,
              filename: "phone.png",
              path: "/data/user/0/com.clipstash.next/files/images/phone.png",
              exists: true,
            },
          ],
        },
      ],
    };
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    const image = await screen.findByRole("img", { name: "phone.png" });
    await user.click(image.closest("button")!);

    const preview = await screen.findByRole("button", { name: "关闭图片预览 phone.png" });
    expect(within(preview).getByRole("img", { name: "phone.png" })).toBeTruthy();
    expect(webviewWindowGetByLabelMock).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "phone.png",
    });

    const backEvent = new Event("clipstash-android-back", { cancelable: true });
    window.dispatchEvent(backEvent);
    expect(backEvent.defaultPrevented).toBe(true);
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "关闭图片预览 phone.png" })).toBeNull();
    });
    expect(webviewWindowGetByLabelMock).not.toHaveBeenCalled();
  });

  it("removes composer images without touching the desktop preview window on Android", async () => {
    const user = userEvent.setup();
    const { default: App } = await import("../src/App");
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "+ 新建" }));
    const composer = await screen.findByRole("dialog", { name: "编辑新消息" });
    await user.upload(
      within(composer).getByLabelText("选择图片"),
      new File([new Uint8Array([1, 2, 3])], "remove.png", { type: "image/png" }),
    );
    expect(await within(composer).findByRole("img", { name: "remove.png" })).toBeTruthy();

    await user.click(within(composer).getByRole("button", { name: "删除图片 remove.png" }));

    expect(within(composer).queryByRole("img", { name: "remove.png" })).toBeNull();
    expect(webviewWindowGetByLabelMock).not.toHaveBeenCalled();
  });
});
