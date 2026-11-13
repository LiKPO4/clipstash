import { controlIconSvg } from "./controlIcons";

export const TEXT_READER_SIZE = { width: 360, height: 500 } as const;

/** Split without losing whitespace or Unicode code points. Measurement is supplied by the rendered paper. */
export function paginateText(text: string, fits: (candidate: string) => boolean): string[] {
  const chars = Array.from(text);
  const pages: string[] = [];
  let start = 0;
  while (start < chars.length) {
    let low = start + 1, high = Math.min(chars.length, start + 16_384), best = start + 1;
    while (low <= high) {
      const end = Math.floor((low + high) / 2);
      if (fits(chars.slice(start, end).join(""))) { best = end; low = end + 1; }
      else high = end - 1;
    }
    if (best < chars.length) {
      const min = start + Math.floor((best - start) * 0.72);
      for (let i = best - 1; i >= min; i--) {
        if (/[\n。！？；]/.test(chars[i])) { best = i + 1; break; }
      }
    }
    pages.push(chars.slice(start, best).join(""));
    start = best;
  }
  return pages.length ? pages : [""];
}

export function mountTextReader(container: HTMLElement, text: string, title: string, onClose: () => void) {
  const reader = document.createElement("section");
  reader.className = "text-reader";
  reader.setAttribute("aria-label", "文本阅读预览");
  // Only fixed UI markup is parsed. Message text and title are assigned with textContent below.
  reader.innerHTML = `<header class="text-reader-toolbar"><span></span><button type="button" aria-label="关闭文本预览">${controlIconSvg("close")}</button></header><div class="text-reader-paper"><header><span>CLIPSTASH / 阅读</span><span>文本预览</span></header><article></article><footer><span>需求暂存站</span><span class="text-reader-page"></span></footer></div><nav class="text-reader-pagination" aria-label="文本预览翻页"><small>滚轮翻页 · Esc 关闭</small><div><button type="button" aria-label="上一页">${controlIconSvg("previous")}</button><button type="button" aria-label="下一页">${controlIconSvg("next")}</button></div></nav>`;
  reader.querySelector(".text-reader-toolbar span")!.textContent = title;
  const body = reader.querySelector("article")!;
  const buttons = reader.querySelectorAll("nav button");
  const prev = buttons[0] as HTMLButtonElement, next = buttons[1] as HTMLButtonElement;
  let pages: string[] = [], index = 0;
  function paint() {
    body.textContent = pages[index];
    reader.querySelector(".text-reader-page")!.textContent = `${index + 1} / ${pages.length}`;
    prev.disabled = index === 0;
    next.disabled = index === pages.length - 1;
  }
  function layout() {
    const previous = index;
    pages = paginateText(text, (candidate) => {
      body.textContent = candidate;
      return body.scrollHeight <= body.clientHeight + 1;
    });
    index = Math.min(previous, pages.length - 1);
    paint();
  }
  prev.onclick = () => { if (index > 0) { index--; paint(); } };
  next.onclick = () => { if (index < pages.length - 1) { index++; paint(); } };
  (reader.querySelector(".text-reader-toolbar button") as HTMLButtonElement).onclick = onClose;
  let wheel = 0, lastWheel = 0;
  reader.addEventListener("wheel", (event) => {
    event.preventDefault();
    const now = Date.now();
    if (now - lastWheel > 200) wheel = 0;
    wheel += event.deltaY;
    if (Math.abs(wheel) < 40 || now - lastWheel < 220) return;
    (wheel > 0 ? next : prev).click();
    wheel = 0; lastWheel = now;
  }, { passive: false });
  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape") onClose();
    if (event.key === "ArrowRight" || event.key === "PageDown") { event.preventDefault(); next.click(); }
    if (event.key === "ArrowLeft" || event.key === "PageUp") { event.preventDefault(); prev.click(); }
  }
  container.append(reader);
  layout();
  const observer = typeof ResizeObserver !== "undefined" ? new ResizeObserver(layout) : null;
  observer?.observe(body);
  window.addEventListener("keydown", keydown);
  return () => { observer?.disconnect(); window.removeEventListener("keydown", keydown); reader.remove(); };
}
