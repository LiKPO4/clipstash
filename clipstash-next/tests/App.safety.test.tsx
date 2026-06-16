import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";

const { invokeMock, isAlwaysOnTopMock, openPathMock, openUrlMock, setAlwaysOnTopMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  isAlwaysOnTopMock: vi.fn(),
  openPathMock: vi.fn(),
  openUrlMock: vi.fn(),
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
  openPath: openPathMock,
  openUrl: openUrlMock,
}));

const stats = {
  data_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash Next",
  db_path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash Next\\clipstash.db",
  images_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash Next\\images",
  db_exists: true,
  images_dir_exists: true,
  normal_count: 6,
  archived_count: 109,
  total_count: 115,
};

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

describe("settings storage panel", () => {
let migratedStats = stats;
let latestReleaseResponse: unknown = null;
  let appSettings = { ...defaultAppSettings };

  beforeEach(() => {
    migratedStats = stats;
    appSettings = { ...defaultAppSettings };
    isAlwaysOnTopMock.mockResolvedValue(false);
    setAlwaysOnTopMock.mockResolvedValue(undefined);
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_app_settings") return Promise.resolve(appSettings);
      if (command === "update_app_settings") {
        return Promise.resolve(appSettings);
      }
      if (command === "get_global_shortcut_errors") return Promise.resolve([]);
      if (command === "get_launch_on_startup") return Promise.resolve(false);
      if (command === "set_launch_on_startup") return Promise.resolve(false);
      if (command === "get_legacy_stats") return Promise.resolve(migratedStats);
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
      if (command === "migrate_legacy_data") {
        migratedStats = { ...stats, normal_count: 8, total_count: 117 };
        return Promise.resolve({
          inserted_messages: 2,
          skipped_messages: 115,
          copied_images: 3,
          skipped_images: 130,
          legacy_message_count: 117,
          legacy_image_count: 133,
          stats: migratedStats,
        });
      }
      if (command === "export_normal_data_zip") {
        return Promise.resolve({
          path: "D:\\clipstash-export.zip",
          message_count: 6,
          image_count: 4,
          skipped_archived_count: 109,
          skipped_missing_image_count: 0,
          skipped_empty_message_count: 0,
          bytes: 2048,
        });
      }
      if (command === "import_data_zip") {
        migratedStats = { ...stats, normal_count: 9, total_count: 118 };
        return Promise.resolve({
          path: "D:\\clipstash-export.zip",
          inserted_messages: 3,
          skipped_messages: 1,
          imported_images: 2,
          stats: migratedStats,
        });
      }
      if (command === "move_app_data_to_selected_dir") {
        migratedStats = {
          ...stats,
          data_dir: "D:\\ClipStashData",
          db_path: "D:\\ClipStashData\\clipstash.db",
          images_dir: "D:\\ClipStashData\\images",
        };
        return Promise.resolve({
          previous_data_dir: stats.data_dir,
          data_dir: migratedStats.data_dir,
          stats: migratedStats,
        });
      }
      if (command === "repair_app_data_dir") {
        return Promise.resolve({
          copied_db: false,
          copied_images: 2,
          skipped_images: 3,
          source_data_dir: stats.data_dir,
          stats: migratedStats,
        });
      }
      if (command === "open_app_path") return Promise.resolve(undefined);
      if (command === "download_and_open_update_installer") {
        return Promise.resolve({
          installer_path: "C:\\Temp\\ClipStash Next_2.0.10_x64-setup.exe",
        });
      }
      if (command === "fetch_latest_github_release") {
        if (latestReleaseResponse instanceof Error) {
          return Promise.reject(latestReleaseResponse);
        }
        return Promise.resolve(latestReleaseResponse);
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    latestReleaseResponse = null;
    localStorage.clear();
    vi.unstubAllGlobals();
    invokeMock.mockReset();
    isAlwaysOnTopMock.mockReset();
    openPathMock.mockReset();
    openUrlMock.mockReset();
    setAlwaysOnTopMock.mockReset();
  });

  it("shows local storage paths without legacy audit actions", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });

    expect(within(dialog).getAllByText("设置")).toHaveLength(1);
    const panel = within(dialog).getByRole("region", { name: "本地存储" });
    expect(within(panel).getByText("数据目录")).toBeTruthy();
    expect(within(panel).getByText("数据库")).toBeTruthy();
    expect(within(panel).getByText("图片目录")).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "导出数据" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "导入数据" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "打开数据目录" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "打开图片目录" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "迁移旧数据" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "迁移数据目录" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "修复数据目录" })).toBeTruthy();
    expect(within(dialog).queryByRole("button", { name: "刷新审计" })).toBeNull();
    expect(invokeMock).not.toHaveBeenCalledWith("get_legacy_safety_report");
  });

  it("runs manual migration and reports skipped duplicates", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    await user.click(within(dialog).getByRole("button", { name: "迁移旧数据" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("migrate_legacy_data");
    });
    expect(await within(dialog).findByText("新增 2 条，跳过 115 条重复，复制图片 3 张。")).toBeTruthy();
    expect(await within(dialog).findByText("已迁移 2 条，跳过 115 条重复")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal",
      sort: "newest",
      offset: 0,
      limit: 30,
    });

  });

  it("exports and imports data packages from settings", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    const panel = within(dialog).getByRole("region", { name: "本地存储" });

    await user.click(within(panel).getByRole("button", { name: "导出数据" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("export_normal_data_zip");
    });
    expect(await within(panel).findByText("已导出 6 条普通消息，图片 4 张。")).toBeTruthy();

    await user.click(within(panel).getByRole("button", { name: "导入数据" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("import_data_zip");
    });
    expect(await within(panel).findByText("已导入 3 条，跳过 1 条重复，图片 2 张。")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal",
      sort: "newest",
      offset: 0,
      limit: 30,
    });
  });

  it("opens storage paths through backend and refreshes paths after moving app data", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    const panel = within(dialog).getByRole("region", { name: "本地存储" });

    await user.click(within(panel).getByRole("button", { name: "打开数据目录" }));
    expect(invokeMock).toHaveBeenCalledWith("open_app_path", { path: stats.data_dir });

    await user.click(within(panel).getByRole("button", { name: "打开图片目录" }));
    expect(invokeMock).toHaveBeenCalledWith("open_app_path", { path: stats.images_dir });

    await user.click(within(panel).getByRole("button", { name: "迁移数据目录" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("move_app_data_to_selected_dir");
    });
    expect(await within(dialog).findByText("数据目录已迁移，原目录已保留")).toBeTruthy();
    expect(await within(panel).findByText("D:\\ClipStashData")).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal",
      sort: "newest",
      offset: 0,
      limit: 30,
    });

    await user.click(within(panel).getByRole("button", { name: "修复数据目录" }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("repair_app_data_dir");
    });
    expect(await within(dialog).findByText("已修复数据目录，补回 2 张图片")).toBeTruthy();
  });

  it("auto-saves settings changes without save or cancel buttons", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });

    expect(within(dialog).queryByRole("button", { name: "保存" })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: "取消" })).toBeNull();

    await user.selectOptions(within(dialog).getByLabelText("消息排序"), "oldest");
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { sort: "oldest" },
    });
    expect(await within(dialog).findByText("消息排序已应用")).toBeTruthy();

    await user.click(within(dialog).getAllByRole("checkbox")[0]);
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { archive_after_import: true },
    });

    await user.click(within(dialog).getAllByRole("checkbox")[1]);
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { close_to_tray: false },
    });

    fireEvent.keyDown(within(dialog).getByLabelText("呼出界面快捷键"), {
      key: "K",
      ctrlKey: true,
      shiftKey: true,
    });
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { show_hotkey: "Ctrl+Shift+K" },
    });
    expect(await within(dialog).findByText("呼出界面快捷键已应用")).toBeTruthy();

    fireEvent.keyDown(within(dialog).getByLabelText("导入当前剪切板快捷键"), {
      key: "L",
      altKey: true,
      ctrlKey: true,
    });
    expect(invokeMock).toHaveBeenCalledWith("update_app_settings", {
      patch: { capture_hotkey: "Ctrl+Alt+L" },
    });
    expect(await within(dialog).findByText("导入剪切板快捷键已应用")).toBeTruthy();

    latestReleaseResponse = {
      tag_name: "v2.1.7",
      html_url: "https://github.com/LiKPO4/clipstash/releases/tag/v2.1.7",
      body: "更新说明",
      assets: [
        {
          name: "ClipStash Next_2.1.7_x64_en-US.msi",
          browser_download_url:
            "https://github.com/LiKPO4/clipstash/releases/download/v2.1.7/ClipStash.Next_2.1.7_x64_en-US.msi",
        },
        {
          name: "ClipStash Next_2.1.7_x64-setup.exe",
          browser_download_url:
            "https://github.com/LiKPO4/clipstash/releases/download/v2.1.7/ClipStash.Next_2.1.7_x64-setup.exe",
        },
      ],
    };

    await user.click(within(dialog).getByRole("button", { name: "检查更新" }));
    expect(await within(dialog).findByText("发现新版本 2.1.7")).toBeTruthy();
    expect(within(dialog).queryByText("更新说明")).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "下载更新" }));
    expect(invokeMock).toHaveBeenCalledWith("download_and_open_update_installer", {
      downloadUrl:
        "https://github.com/LiKPO4/clipstash/releases/download/v2.1.7/ClipStash.Next_2.1.7_x64-setup.exe",
      filename: "ClipStash Next_2.1.7_x64-setup.exe",
    });
    expect(await within(dialog).findByText("安装包已打开，请按安装向导完成更新")).toBeTruthy();
  });

  it("shows concrete update check errors for network, http, and malformed release responses", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    const checkButton = within(dialog).getByRole("button", { name: "检查更新" });

    latestReleaseResponse = new Error("网络不可用");
    await user.click(checkButton);
    expect(await within(dialog).findByText("网络不可用")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "打开 Release 页面" }));
    expect(openUrlMock).toHaveBeenCalledWith("https://github.com/LiKPO4/clipstash/releases/latest");

    latestReleaseResponse = new Error("GitHub Release 检查失败：HTTP 503");
    await user.click(checkButton);
    expect(await within(dialog).findByText("GitHub Release 检查失败：HTTP 503")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "打开 Release 页面" })).toBeTruthy();

    latestReleaseResponse = {
      html_url: "https://github.com/LiKPO4/clipstash/releases/latest",
      body: "缺少 tag",
    };
    await user.click(checkButton);
    expect(await within(dialog).findByText("GitHub Release 响应缺少版本号")).toBeTruthy();
    expect(within(dialog).queryByText("待实现")).toBeNull();
  });
});
