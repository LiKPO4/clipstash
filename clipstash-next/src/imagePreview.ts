import { emitTo, listen } from "@tauri-apps/api/event";
import { mountTextReader } from "./textReading";
import "./textReading.css";
import "./buttonFeedback.css";

let image = document.querySelector<HTMLImageElement>("#preview-image")!;
let generation = 0;
let disposeText: (() => void) | undefined;
function clear() {
  disposeText?.(); disposeText = undefined;
  image.hidden = false;
  generation += 1;
  image.onload = null;
  image.onerror = null;
  image.classList.remove("ready");
  image.removeAttribute("src");
}
async function start() {
  await listen("clipstash-preview-clear", clear);
  await listen<{ id: number; filename: string; src: string; text?: string }>("clipstash-preview-load", ({ payload }) => {
    clear();
    if (payload.text !== undefined) {
      image.hidden = true;
      disposeText = mountTextReader(document.body, payload.text, payload.filename, () => { void emitTo("main", "clipstash-text-reader", { close: true }); });
      void emitTo("main", "clipstash-preview-loaded", { id: payload.id });
      return;
    }
    const next = image.cloneNode(false) as HTMLImageElement;
    image.replaceWith(next);
    image = next;
    const current = generation;
    image.alt = payload.filename;
    image.onload = () => {
      if (current !== generation) return;
      image.classList.add("ready");
      void emitTo("main", "clipstash-preview-loaded", { id: payload.id });
    };
    image.onerror = () => {
      if (current === generation) void emitTo("main", "clipstash-preview-loaded", { id: payload.id, error: "原图加载失败" });
    };
    image.src = payload.src;
  });
  document.body.addEventListener("mouseenter", () => { if (disposeText) void emitTo("main", "clipstash-text-reader", { hovered: true }); });
  document.body.addEventListener("mouseleave", () => { if (disposeText) void emitTo("main", "clipstash-text-reader", { hovered: false }); });
  await listen("clipstash-preview-ping", () => { void emitTo("main", "clipstash-preview-ready"); });
  await emitTo("main", "clipstash-preview-ready");
}
void start();
