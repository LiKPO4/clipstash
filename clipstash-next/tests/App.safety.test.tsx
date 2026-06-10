import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
  paste_interval_ms: 250,
  show_hotkey: "<ctrl>+<shift>+v",
  capture_hotkey: "<ctrl>+<alt>+v",
  hover_delay: 0.8,
  scroll_lines: 1,
  font_scale: 0,
  edit_textarea_height: 360,
  sort: "newest",
};

describe("settings storage panel", () => {
  let migratedStats = stats;
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
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
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

    const panel = within(dialog).getByRole("region", { name: "本地存储" });
    expect(within(panel).getByText("数据目录")).toBeTruthy();
    expect(within(panel).getByText("数据库")).toBeTruthy();
    expect(within(panel).getByText("图片目录")).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "打开数据目录" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "打开图片目录" })).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "迁移旧数据" })).toBeTruthy();
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

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          tag_name: "v2.0.5",
          html_url: "https://github.com/LiKPO4/clipstash/releases/tag/v2.0.5",
          body: "更新说明",
        }),
      }),
    );

    await user.click(within(dialog).getByRole("button", { name: "检查更新" }));
    expect(await within(dialog).findByText("发现新版本 2.0.5")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "打开 Release 页面" }));
    expect(openUrlMock).toHaveBeenCalledWith(
      "https://github.com/LiKPO4/clipstash/releases/tag/v2.0.5",
    );
  });

  it("shows concrete update check errors for network, http, and malformed release responses", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "设置" }));
    const dialog = await screen.findByRole("dialog", { name: "设置" });
    const checkButton = within(dialog).getByRole("button", { name: "检查更新" });

    vi.stubGlobal("fetch", vi.fn().mockRejectedValueOnce(new Error("网络不可用")));
    await user.click(checkButton);
    expect(await within(dialog).findByText("网络不可用")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "打开 Release 页面" }));
    expect(openUrlMock).toHaveBeenCalledWith("https://github.com/LiKPO4/clipstash/releases/latest");

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValueOnce({
        ok: false,
        status: 503,
        json: async () => ({}),
      }),
    );
    await user.click(checkButton);
    expect(await within(dialog).findByText("GitHub Release 检查失败：HTTP 503")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "打开 Release 页面" })).toBeTruthy();

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          html_url: "https://github.com/LiKPO4/clipstash/releases/latest",
          body: "缺少 tag",
        }),
      }),
    );
    await user.click(checkButton);
    expect(await within(dialog).findByText("GitHub Release 响应缺少版本号")).toBeTruthy();
    expect(within(dialog).queryByText("待实现")).toBeNull();
  });
});
