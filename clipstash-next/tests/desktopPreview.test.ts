import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  callbacks: new Map<string, (event: { payload: any }) => void>(),
  invoke: vi.fn(), emit: vi.fn(), create: vi.fn(), show: vi.fn(), hide: vi.fn(),
  close: vi.fn(), size: vi.fn(), position: vi.fn(), title: vi.fn(), autoLoad: true,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke, convertFileSrc: (path: string) => `asset://${path}` }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: any }) => void) => {
    mocks.callbacks.set(name, callback);
    return () => mocks.callbacks.delete(name);
  }),
  emitTo: mocks.emit,
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    static getByLabel = vi.fn(async () => null);
    constructor(label: string, options: unknown) {
      mocks.create(label, options);
      queueMicrotask(() => mocks.callbacks.get("clipstash-preview-ready")?.({ payload: null }));
    }
    once = vi.fn(async () => () => undefined);
    show = mocks.show; hide = mocks.hide; close = mocks.close;
    setSize = mocks.size; setPosition = mocks.position; setTitle = mocks.title;
  },
}));
const position = (width: number, height: number) => ({ left: 10, top: 20, width, height });
const source = { filename: "one.png", path: "C:/synthetic/images/one.png" };
beforeEach(() => {
  vi.resetModules();
  mocks.callbacks.clear();
  for (const value of Object.values(mocks)) if (vi.isMockFunction(value)) value.mockReset();
  for (const command of [mocks.show, mocks.hide, mocks.close, mocks.size, mocks.position, mocks.title]) command.mockResolvedValue(undefined);
  mocks.autoLoad = true;
  mocks.invoke.mockResolvedValue({ path: source.path, width: 640, height: 480, lease: null });
  mocks.emit.mockImplementation(async (_label, event, payload) => {
    if (event === "clipstash-preview-load" && mocks.autoLoad)
      queueMicrotask(() => mocks.callbacks.get("clipstash-preview-loaded")?.({ payload: { id: payload.id } }));
  });
});
afterEach(() => vi.restoreAllMocks());

describe("desktop preview protocol", () => {
  it("reuses a non-focusing window and passes a URL instead of original bytes or localStorage", async () => {
    const storage = vi.spyOn(Storage.prototype, "setItem");
    const { showDesktopPreview, hideDesktopPreview } = await import("../src/desktopPreview");
    await showDesktopPreview(source, position);
    await hideDesktopPreview();
    const hides = mocks.hide.mock.calls.length;
    await hideDesktopPreview();
    expect(mocks.hide).toHaveBeenCalledTimes(hides);
    await showDesktopPreview({ ...source, filename: "two.png" }, position);
    expect(mocks.create).toHaveBeenCalledTimes(1);
    expect(mocks.create).toHaveBeenCalledWith("image-preview", expect.objectContaining({ visible: false, focus: false, focusable: false }));
    expect(mocks.show).toHaveBeenCalledTimes(2);
    expect(mocks.close).not.toHaveBeenCalled();
    expect(mocks.invoke.mock.calls.every(([command]) => command === "prepare_image_preview")).toBe(true);
    expect(mocks.emit).toHaveBeenCalledWith("image-preview", "clipstash-preview-load", expect.objectContaining({ src: expect.stringMatching(/^asset:\/\//) }));
    expect(storage).not.toHaveBeenCalled();
    await hideDesktopPreview();
  });

  it("cancels a slow load and ignores its late completion when a newer image is requested", async () => {
    mocks.autoLoad = false;
    const { showDesktopPreview, hideDesktopPreview } = await import("../src/desktopPreview");
    const first = showDesktopPreview(source, position);
    await vi.waitFor(() => expect(mocks.emit.mock.calls.some((call) => call[1] === "clipstash-preview-load")).toBe(true));
    const oldId = mocks.emit.mock.calls.find((call) => call[1] === "clipstash-preview-load")![2].id;
    mocks.autoLoad = true;
    const second = showDesktopPreview({ ...source, filename: "new.png" }, position);
    await Promise.all([first, second]);
    mocks.callbacks.get("clipstash-preview-loaded")?.({ payload: { id: oldId } });
    expect(mocks.show).toHaveBeenCalledTimes(1);
    expect(mocks.title).toHaveBeenLastCalledWith("new.png");
    expect(mocks.create).toHaveBeenCalledTimes(1);
    await hideDesktopPreview();
  });

  it("does not show a window after closing during original preparation", async () => {
    let resolve!: (value: unknown) => void;
    mocks.invoke.mockImplementation(() => new Promise((done) => { resolve = done; }));
    const { showDesktopPreview, hideDesktopPreview } = await import("../src/desktopPreview");
    const showing = showDesktopPreview(source, position);
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalled());
    const closing = hideDesktopPreview();
    resolve({ path: source.path, width: 640, height: 480, lease: null });
    await Promise.all([showing, closing]);
    expect(mocks.show).not.toHaveBeenCalled();
  });

  it("uploads a new File as binary and releases its temporary lease on close", async () => {
    mocks.invoke.mockImplementation(async (command) => command === "prepare_preview_upload"
      ? { path: "C:/cache/preview.bin", width: 20, height: 40, lease: "upload-1" } : undefined);
    const file = new File([new Uint8Array([1, 2, 3])], "local.png");
    Object.defineProperty(file, "arrayBuffer", { value: async () => new Uint8Array([1, 2, 3]).buffer });
    const { showDesktopPreview, hideDesktopPreview } = await import("../src/desktopPreview");
    await showDesktopPreview({ ...source, file }, position);
    expect(mocks.invoke).toHaveBeenCalledWith("prepare_preview_upload", new Uint8Array([1, 2, 3]));
    await hideDesktopPreview();
    expect(mocks.invoke).toHaveBeenCalledWith("release_preview_upload", { lease: "upload-1" });
  });

  it("clears failed image loads and can reuse the window for a subsequent request", async () => {
    mocks.emit.mockImplementationOnce(async () => undefined);
    mocks.emit.mockImplementation(async (_label, event, payload) => {
      if (event === "clipstash-preview-load") queueMicrotask(() =>
        mocks.callbacks.get("clipstash-preview-loaded")?.({ payload: { id: payload.id, error: "bad image" } }));
    });
    const { showDesktopPreview, hideDesktopPreview } = await import("../src/desktopPreview");
    await expect(showDesktopPreview(source, position)).rejects.toThrow("bad image");
    expect(mocks.show).not.toHaveBeenCalled();
    expect(mocks.emit).toHaveBeenLastCalledWith("image-preview", "clipstash-preview-clear");
    mocks.emit.mockImplementation(async (_label, event, payload) => {
      if (event === "clipstash-preview-load") queueMicrotask(() =>
        mocks.callbacks.get("clipstash-preview-loaded")?.({ payload: { id: payload.id } }));
    });
    await showDesktopPreview(source, position);
    expect(mocks.create).toHaveBeenCalledTimes(1);
    expect(mocks.show).toHaveBeenCalledTimes(1);
    await hideDesktopPreview();
  });
});
