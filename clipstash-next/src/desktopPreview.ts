import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ReusablePreview, type PreviewPosition, type PreviewWindowPort } from "./reusablePreview";

type Metadata = { path: string; width: number; height: number; lease: string | null };
export type DesktopPreviewSource = { filename: string; path: string; file?: File };
const label = "image-preview";

async function createPreviewWindow(onDestroyed: () => void): Promise<PreviewWindowPort> {
  let readyResolve!: () => void;
  let readyReject!: (error: unknown) => void;
  const ready = new Promise<void>((resolve, reject) => { readyResolve = resolve; readyReject = reject; });
  void ready.catch(() => undefined);
  // Attach before creation: the preview page may finish loading before tauri://created fires.
  const unlistenReady = await listen("clipstash-preview-ready", () => readyResolve());
  const timer = window.setTimeout(() => readyReject(new Error("预览窗口启动超时")), 8000);
  let nativeWindow: WebviewWindow | null = null;
  let pending: { id: number; resolve: () => void; reject: (error: Error) => void; timer: number } | null = null;
  const cancelLoad = () => {
    if (!pending) return;
    window.clearTimeout(pending.timer);
    pending.reject(new Error("预览请求已取消"));
    pending = null;
  };
  let unlistenLoaded = () => {};
  let unlistenError = () => {};
  try {
    unlistenLoaded = await listen<{ id: number; error?: string }>("clipstash-preview-loaded", ({ payload }) => {
    if (!pending || payload.id !== pending.id) return;
    window.clearTimeout(pending.timer);
    if (payload.error) pending.reject(new Error(payload.error));
    else pending.resolve();
    pending = null;
    });
    nativeWindow = await WebviewWindow.getByLabel(label);
    if (nativeWindow) {
      await emitTo(label, "clipstash-preview-ping");
    } else {
      nativeWindow = new WebviewWindow(label, {
        alwaysOnTop: true, decorations: false, focus: false, focusable: false,
        height: 240, width: 320, resizable: false, skipTaskbar: true,
        title: "图片预览", transparent: true, url: "/image-preview.html", visible: false,
      });
      unlistenError = await nativeWindow.once("tauri://error", ({ payload }) => readyReject(new Error(String(payload))));
    }
    await ready;
  } catch (error) {
    unlistenLoaded();
    await nativeWindow?.close().catch(() => undefined);
    throw error;
  } finally {
    window.clearTimeout(timer);
    unlistenReady();
    unlistenError();
  }
  const win = nativeWindow;
  await win.once("tauri://destroyed", () => { cancelLoad(); unlistenLoaded(); onDestroyed(); });
  return {
    hide: () => win.hide(),
    clear: () => emitTo(label, "clipstash-preview-clear"),
    cancelLoad,
    configure: async (position, title) => {
      await win.setSize(new LogicalSize(position.width, position.height));
      await win.setPosition(new LogicalPosition(position.left, position.top));
      await win.setTitle(title);
    },
    load: (id, source) => {
      cancelLoad();
      return new Promise<void>((resolve, reject) => {
        pending = { id, resolve, reject,
          timer: window.setTimeout(() => { cancelLoad(); }, 8000) };
        void emitTo(label, "clipstash-preview-load", { id, filename: source.filename, src: source.src })
          .catch(() => cancelLoad());
      });
    },
    show: () => win.show(),
  };
}

const preview = new ReusablePreview(createPreviewWindow);
export function showDesktopPreview(source: DesktopPreviewSource, position: (width: number, height: number) => PreviewPosition) {
  return preview.show(async () => {
    if (source.file && source.file.size > 128 * 1024 * 1024) throw new Error("临时预览图片超过 128MiB");
    const metadata: Metadata = source.file
      ? await invoke("prepare_preview_upload", new Uint8Array(await source.file.arrayBuffer()))
      : await invoke("prepare_image_preview", { filename: source.filename, expectedPath: source.path });
    return { ...metadata, filename: source.filename,
      src: `${convertFileSrc(metadata.path)}?preview=${encodeURIComponent(metadata.lease ?? String(Date.now()))}`,
      release: async () => { if (metadata.lease) await invoke("release_preview_upload", { lease: metadata.lease }); },
    };
  }, position);
}
export function hideDesktopPreview() { return preview.hide(); }
