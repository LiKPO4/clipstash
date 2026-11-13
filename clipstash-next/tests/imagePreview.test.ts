import { beforeEach, afterEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ callbacks: new Map<string, (event?: any) => void>(), emit: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  emitTo: mocks.emit,
  listen: async (name: string, callback: (event?: any) => void) => { mocks.callbacks.set(name, callback); return () => {}; },
}));
beforeEach(() => { vi.resetModules(); mocks.callbacks.clear(); mocks.emit.mockReset(); document.body.innerHTML = '<img id="preview-image" />'; });
afterEach(() => { document.body.innerHTML = ""; });

it("acknowledges text without image decoding, clears the paper, and restores image rendering", async () => {
  await import("../src/imagePreview");
  await vi.waitFor(() => expect(mocks.emit).toHaveBeenCalledWith("main", "clipstash-preview-ready"));
  mocks.callbacks.get("clipstash-preview-load")!({ payload: { id: 10, filename: "消息 #1", src: "", text: "第一行\n<b>原文</b>" } });
  expect(document.querySelector("img")!.hidden).toBe(true);
  expect(document.querySelector("article")!.textContent).toBe("第一行\n<b>原文</b>");
  expect(document.querySelector("b")).toBeNull();
  expect(mocks.emit).toHaveBeenCalledWith("main", "clipstash-preview-loaded", { id: 10 });
  mocks.callbacks.get("clipstash-preview-load")!({ payload: { id: 11, filename: "next.png", src: "asset://next" } });
  expect(document.querySelector(".text-reader")).toBeNull();
  const image = document.querySelector("img")!;
  expect(image.hidden).toBe(false);
  image.dispatchEvent(new Event("load"));
  expect(mocks.emit).toHaveBeenCalledWith("main", "clipstash-preview-loaded", { id: 11 });
  mocks.callbacks.get("clipstash-preview-clear")!();
});

it("acknowledges only the latest image and removes its resource on clear", async () => {
  await import("../src/imagePreview");
  await vi.waitFor(() => expect(mocks.emit).toHaveBeenCalledWith("main", "clipstash-preview-ready"));
  mocks.callbacks.get("clipstash-preview-load")!({ payload: { id: 1, filename: "one", src: "asset://one" } });
  const old = document.querySelector("img")!;
  const lateLoad = old.onload!;
  mocks.callbacks.get("clipstash-preview-load")!({ payload: { id: 2, filename: "two", src: "asset://two" } });
  lateLoad.call(old, new Event("load"));
  expect(mocks.emit).not.toHaveBeenCalledWith("main", "clipstash-preview-loaded", { id: 1 });
  const current = document.querySelector("img")!;
  expect(current).not.toBe(old);
  current.dispatchEvent(new Event("load"));
  expect(mocks.emit).toHaveBeenCalledWith("main", "clipstash-preview-loaded", { id: 2 });
  expect(current.classList.contains("ready")).toBe(true);
  mocks.callbacks.get("clipstash-preview-clear")!();
  expect(current.hasAttribute("src")).toBe(false);
  expect(current.classList.contains("ready")).toBe(false);
});
