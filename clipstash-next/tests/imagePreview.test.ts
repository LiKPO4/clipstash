import { beforeEach, afterEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({ callbacks: new Map<string, (event?: any) => void>(), emit: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  emitTo: mocks.emit,
  listen: async (name: string, callback: (event?: any) => void) => { mocks.callbacks.set(name, callback); return () => {}; },
}));
beforeEach(() => { vi.resetModules(); mocks.callbacks.clear(); mocks.emit.mockReset(); document.body.innerHTML = '<img id="preview-image" />'; });
afterEach(() => { document.body.innerHTML = ""; });

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
