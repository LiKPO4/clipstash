import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { canShareMock, invokeMock, isAlwaysOnTopMock, openPathMock, setAlwaysOnTopMock, shareMock } = vi.hoisted(() => ({
  canShareMock: vi.fn(),
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
  openPathMock: vi.fn(),
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
  let androidDownloadAndInstallApkMock: ReturnType<typeof vi.fn>;
  let androidRefreshWidgetsMock: ReturnType<typeof vi.fn>;
  let androidShareZipMock: ReturnType<typeof vi.fn> | null = null;

  beforeEach(() => {
    vi.resetModules();
    appSettings = { ...defaultAppSettings };
    listedPage = normalPage;
    androidCheckForUpdatesMock = vi.fn().mockReturnValue(true);
    androidDownloadAndInstallApkMock = vi.fn().mockReturnValue(true);
    androidRefreshWidgetsMock = vi.fn();
    androidShareZipMock = null;
    window.ClipStashAndroid = {
      checkForUpdates: androidCheckForUpdatesMock,
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
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "update_app_settings") {
        appSettings = { ...appSettings, ...(args?.patch as Record<string, unknown>) };
        return Promise.resolve(appSettings);
      }
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") return Promise.resolve(listedPage);
      if (command === "read_legacy_image_bytes") return Promise.resolve([]);
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
    setAlwaysOnTopMock.mockReset();
    canShareMock.mockReset();
    openPathMock.mockReset();
    shareMock.mockReset();
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
            tag_name: "v2.1.14",
            html_url: "https://github.com/LiKPO4/clipstash/releases/tag/v2.1.14",
            assets: [
              {
                name: "ClipStash.Next_2.1.14_android-universal-release-signed.apk",
                browser_download_url: "https://github.com/LiKPO4/clipstash/releases/download/v2.1.14/ClipStash.Next_2.1.14_android-universal-release-signed.apk",
              },
            ],
          },
        },
      }));
    });
    expect(await within(dialog).findByText("发现新版本 2.1.14")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "下载并安装" }));
    expect(androidDownloadAndInstallApkMock).toHaveBeenCalledWith(
      "https://github.com/LiKPO4/clipstash/releases/download/v2.1.14/ClipStash.Next_2.1.14_android-universal-release-signed.apk",
      "ClipStash.Next_2.1.14_android-universal-release-signed.apk",
    );

    await user.click(within(dialog).getByRole("button", { name: "关闭设置" }));
    await user.click(screen.getByRole("button", { name: "+ 新建" }));
    const composer = await screen.findByRole("dialog", { name: "编辑新消息" });
    expect(within(composer).getByLabelText("关闭新消息")).toBeTruthy();
    expect(composer.querySelector(".edit-dialog-actions")).toBeNull();
    expect(within(composer).getByLabelText("选择图片")).toBeTruthy();
    expect(within(composer).queryByRole("button", { name: "关闭" })).toBeNull();
    expect(within(composer).getByRole("button", { name: "保存" })).toBeTruthy();
    expect((within(composer).getByLabelText("消息内容") as HTMLTextAreaElement).style.fontSize).toBe("1.5em");

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

  it("creates a message from android shared text", async () => {
    window.ClipStashAndroid = {
      consumePendingShare: vi.fn().mockReturnValue(JSON.stringify({ text: "  分享文字  ", images: [] })),
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
        .mockReturnValue(JSON.stringify({ text: "", images: [{ mimeType: "image/png", data: "AQID" }] })),
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
    expect(invokeMock).not.toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "phone.png",
    });

    await user.click(within(preview).getByRole("img", { name: "phone.png" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "关闭图片预览 phone.png" })).toBeNull();
    });
  });
});
