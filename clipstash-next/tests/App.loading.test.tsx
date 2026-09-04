import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";
import type { LegacyMessage, LegacyMessagePage } from "../src/api/types";
import { imageUrlMocks, installImageUrlMocks } from "./imageUrlMocks";

installImageUrlMocks();

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    isAlwaysOnTop: vi.fn().mockResolvedValue(false),
    setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static getByLabel = vi.fn().mockResolvedValue(null);
    close = vi.fn().mockResolvedValue(undefined);
    once = vi.fn().mockResolvedValue(vi.fn());
  },
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn(), openUrl: vi.fn() }));

function message(id: number, imageCount = 1): LegacyMessage {
  return {
    id, text_content: `测试消息 ${id}`, created_at: "2026-09-04 00:00:00",
    archived: false, archived_at: null,
    images: Array.from({ length: imageCount }, (_, index) => ({
      id: id * 10 + index, filename: `${id}-${index}.png`,
      path: `C:/synthetic/images/${id}-${index}.png`, exists: true,
    })),
  };
}

function page(messages: LegacyMessage[], extra: Partial<LegacyMessagePage> = {}): LegacyMessagePage {
  return { view: "normal", sort: "newest", offset: 0, limit: 30,
    total_count: messages.length, has_more: false, messages, ...extra };
}

const settings = {
  always_on_top: false, close_to_tray: true, launch_on_startup: false,
  main_window_state: null, archive_after_import: false, archive_after_export: false,
  match_blank_lines_to_images: false, message_double_click_action: "edit",
  paste_interval_ms: 250, show_hotkey: "Ctrl+Shift+V", capture_hotkey: "Ctrl+Alt+V",
  hover_delay: 0.8, scroll_lines: 1, font_scale: 0, edit_textarea_height: 360, sort: "newest",
};

type PendingImage = { filename: string; resolve: (value: Uint8Array) => void; reject: (reason: Error) => void };
let pending: PendingImage[];
let getPage: (args: { view?: string; offset?: number; limit?: number }) => LegacyMessagePage | Promise<LegacyMessagePage>;
let active: number;
let maxActive: number;
const bytes = new Uint8Array([137, 80, 78, 71]);
let intersectionObserverDescriptor: PropertyDescriptor | undefined;

function installTestIntersectionObserver() {
  const callbacks: IntersectionObserverCallback[] = [];
  class TestIntersectionObserver {
    constructor(callback: IntersectionObserverCallback) {
      callbacks.push(callback);
    }
    observe() {}
    disconnect() {}
    takeRecords() { return []; }
    root = null;
    rootMargin = "0px";
    thresholds = [];
  }
  intersectionObserverDescriptor = Object.getOwnPropertyDescriptor(window, "IntersectionObserver");
  Object.defineProperty(window, "IntersectionObserver", {
    configurable: true,
    value: TestIntersectionObserver,
  });
  return callbacks;
}

beforeEach(() => {
  pending = [];
  active = 0;
  maxActive = 0;
  getPage = () => page([message(1)]);
  invokeMock.mockImplementation((command: string, args: { filename: string; view?: string; offset?: number }) => {
    if (command === "get_app_settings" || command === "update_app_settings") return Promise.resolve(settings);
    if (command === "get_global_shortcut_errors") return Promise.resolve([]);
    if (command === "get_launch_on_startup") return Promise.resolve(false);
    if (command === "get_legacy_stats") return Promise.resolve({
      data_dir: "C:/synthetic", db_path: "C:/synthetic/clipstash.db", images_dir: "C:/synthetic/images",
      db_exists: true, images_dir_exists: true, normal_count: 30, archived_count: 30, total_count: 60,
    });
    if (command === "list_legacy_messages") return Promise.resolve(getPage(args));
    if (command === "read_image_thumbnail_bytes") {
      active += 1;
      maxActive = Math.max(maxActive, active);
      return new Promise<Uint8Array>((resolve, reject) => {
        pending.push({ filename: args.filename, resolve, reject });
      }).finally(() => { active -= 1; });
    }
    return Promise.reject(new Error(`Unexpected command: ${command}`));
  });
});

afterEach(async () => {
  cleanup();
  await act(async () => { pending.forEach((request) => request.resolve(bytes)); });
  if (intersectionObserverDescriptor) {
    Object.defineProperty(window, "IntersectionObserver", intersectionObserverDescriptor);
  } else {
    delete (window as Window & { IntersectionObserver?: typeof IntersectionObserver }).IntersectionObserver;
  }
  intersectionObserverDescriptor = undefined;
  localStorage.clear();
  invokeMock.mockReset();
});

describe("message loading independent of image IO", () => {
  it("refreshes the loaded prefix after archiving without collapsing back to 30 rows", async () => {
    let rows = Array.from({ length: 90 }, (_, index) => message(index + 1, 0));
    getPage = ({ offset = 0, limit = 30 }) => page(rows.slice(offset, offset + limit), {
      offset, limit, total_count: rows.length, has_more: offset + limit < rows.length,
    });
    const previous = invokeMock.getMockImplementation()!;
    invokeMock.mockImplementation((command, args) => {
      if (command === "set_legacy_message_archived") {
        rows = rows.filter((row) => row.id !== args.messageId);
        return Promise.resolve({ message: { ...message(args.messageId, 0), archived: true } });
      }
      return previous(command, args);
    });
    render(<App />);
    await screen.findByText("测试消息 30");
    const list = screen.getByRole("region", { name: "消息列表" });
    fireEvent.scroll(list);
    await screen.findByText("测试消息 60");
    list.scrollTop = 1200;
    fireEvent.click(screen.getAllByRole("button", { name: "归档" })[0]);
    await screen.findByText("测试消息 61");
    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal", sort: "newest", offset: 0, limit: 60,
    });
    expect(list.scrollTop).toBe(1200);
    expect(list.querySelectorAll(".message-card")).toHaveLength(60);
  });

  it("shows text and controls while image IO is pending, then fills the image", async () => {
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending.length).toBe(1));
    expect((screen.getByRole("button", { name: "已归档" }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByText("无法读取")).toBeNull();
    expect(screen.queryByAltText("1-0.png")).toBeNull();
    await act(async () => pending[0].resolve(bytes));
    expect(await screen.findByAltText("1-0.png")).toBeTruthy();
    expect(screen.getByText("测试消息 1")).toBeTruthy();
  });

  it("isolates a failed image from text and another successfully loaded image", async () => {
    getPage = () => page([message(1, 2)]);
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending.length).toBe(2));
    await act(async () => {
      pending[0].reject(new Error("synthetic read failure"));
      pending[1].resolve(bytes);
    });
    expect(await screen.findByAltText("1-1.png")).toBeTruthy();
    expect(screen.getByText("无法读取")).toBeTruthy();
    expect(screen.getByText("测试消息 1")).toBeTruthy();
  });

  it("appends the next page without waiting for either page's images", async () => {
    getPage = ({ offset }) => offset
      ? page([message(2)], { offset: 1, total_count: 2 })
      : page([message(1)], { total_count: 2, has_more: true });
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending.length).toBe(1));
    fireEvent.scroll(screen.getByRole("region", { name: "消息列表" }));
    expect(await screen.findByText("测试消息 2")).toBeTruthy();
    expect(screen.getByText("测试消息 1")).toBeTruthy();
    expect(screen.queryByAltText("2-0.png")).toBeNull();
    expect(invokeMock.mock.calls.some(([command, args]) => command === "list_legacy_messages" && args.offset === 1)).toBe(true);
  });

  it("does not start obsolete queued thumbnails after switching views, or overwrite the new page", async () => {
    getPage = ({ view }) => view === "archived"
      ? page([message(2)], { view: "archived" })
      : page([message(1), message(3), message(4), message(5), message(6)]);
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending.length).toBe(4));
    fireEvent.click(screen.getByRole("button", { name: "已归档" }));
    expect(await screen.findByText("测试消息 2")).toBeTruthy();
    await act(async () => { pending.slice(0, 4).forEach((request) => request.resolve(bytes)); });
    await waitFor(() => expect(pending.some((request) => request.filename === "2-0.png")).toBe(true));
    await act(async () => pending.find((request) => request.filename === "2-0.png")!.resolve(bytes));
    expect(await screen.findByAltText("2-0.png")).toBeTruthy();
    expect(screen.queryByText("测试消息 1")).toBeNull();
    expect(pending.some((request) => request.filename === "6-0.png")).toBe(false);
    expect(maxActive).toBeLessThanOrEqual(4);
  });

  it("continues filling the current page if fetching the next page fails", async () => {
    getPage = ({ offset }) => offset
      ? Promise.reject(new Error("synthetic page failure"))
      : page([message(1), message(2), message(3), message(4), message(5)], { total_count: 6, has_more: true });
    render(<App />);
    await waitFor(() => expect(pending.length).toBe(4));
    fireEvent.scroll(screen.getByRole("region", { name: "消息列表" }));
    await screen.findByText("synthetic page failure");
    await act(async () => pending.slice(0, 4).forEach((request) => request.resolve(bytes)));
    expect(await screen.findByAltText("1-0.png")).toBeTruthy();
    await waitFor(() => expect(pending.some((request) => request.filename === "5-0.png")).toBe(true));
    await act(async () => pending.find((request) => request.filename === "5-0.png")!.resolve(bytes));
  });

  it("stops queued reads after unmount", async () => {
    getPage = () => page([message(1), message(2), message(3), message(4), message(5)]);
    const { unmount } = render(<App />);
    await waitFor(() => expect(pending.length).toBe(4));
    unmount();
    await act(async () => pending.forEach((request) => request.resolve(bytes)));
    expect(pending.length).toBe(4);
  });

  it("does not request a collapsed fourth image until the message is expanded", async () => {
    getPage = () => page([message(1, 4)]);
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending.map((request) => request.filename)).toEqual([
      "1-0.png", "1-1.png", "1-2.png",
    ]));
    expect(pending.some((request) => request.filename === "1-3.png")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "展开 1 张图片" }));
    await waitFor(() => expect(pending.some((request) => request.filename === "1-3.png")).toBe(true));
  });

  it("uses a thumbnail for the list and does not read an original before preview", async () => {
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    await waitFor(() => expect(pending).toHaveLength(1));
    await act(async () => pending[0].resolve(bytes));
    expect(await screen.findByAltText("1-0.png")).toBeTruthy();
    expect(invokeMock.mock.calls.some(([command]) => command === "read_legacy_image_bytes")).toBe(false);
    expect(imageUrlMocks.createObjectURL).toHaveBeenCalledTimes(1);
  });

  it("releases the offscreen thumbnail lease and reuses its cached URL on re-entry", async () => {
    const callbacks = installTestIntersectionObserver();
    render(<App />);
    expect(await screen.findByText("测试消息 1")).toBeTruthy();
    expect(pending).toHaveLength(0);
    expect(callbacks).toHaveLength(1);

    await act(async () => {
      callbacks[0]([{ isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver);
    });
    await waitFor(() => expect(pending).toHaveLength(1));
    await act(async () => pending[0].resolve(bytes));
    expect(await screen.findByAltText("1-0.png")).toBeTruthy();

    await act(async () => {
      callbacks[0]([{ isIntersecting: false } as IntersectionObserverEntry], {} as IntersectionObserver);
    });
    await waitFor(() => expect(screen.queryByAltText("1-0.png")).toBeNull());
    await act(async () => {
      callbacks[0]([{ isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver);
    });
    expect(await screen.findByAltText("1-0.png")).toBeTruthy();
    expect(invokeMock.mock.calls.filter(([command]) => command === "read_image_thumbnail_bytes")).toHaveLength(1);
  });
});
