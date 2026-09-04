import { emitTo, listen } from "@tauri-apps/api/event";

let image = document.querySelector<HTMLImageElement>("#preview-image")!;
let generation = 0;
function clear() {
  generation += 1;
  image.onload = null;
  image.onerror = null;
  image.classList.remove("ready");
  image.removeAttribute("src");
}
async function start() {
  await listen("clipstash-preview-clear", clear);
  await listen<{ id: number; filename: string; src: string }>("clipstash-preview-load", ({ payload }) => {
    clear();
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
  await listen("clipstash-preview-ping", () => { void emitTo("main", "clipstash-preview-ready"); });
  await emitTo("main", "clipstash-preview-ready");
}
void start();
