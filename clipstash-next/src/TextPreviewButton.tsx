import { type ComponentProps, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { showDesktopPreview, hideDesktopPreview } from "./desktopPreview";
import { mountTextReader, TEXT_READER_SIZE } from "./textReading";
import type { PreviewPosition } from "./reusablePreview";
import "./textReading.css";

type Props = ComponentProps<"button"> & {
  text: string; title: string; isAndroid: boolean; delay: number;
  position: (anchor: DOMRect, width: number, height: number) => PreviewPosition;
  beforePreview: () => void;
};

/** Retains copy/double-click semantics; desktop hover shares the existing preview window. */
export function TextPreviewButton({ text, title, isAndroid, delay, position, beforePreview, ...props }: Props) {
  const anchor = useRef<HTMLButtonElement>(null);
  const reader = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const closing = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const generation = useRef(0);
  const active = useRef(false);
  const [fallback, setFallback] = useState<PreviewPosition | null>(null);
  function cancel(hide = true) {
    generation.current++;
    clearTimeout(timer.current); clearTimeout(closing.current);
    setFallback(null);
    if (active.current && hide) void hideDesktopPreview().catch(() => undefined);
    active.current = false;
  }
  function leave() {
    clearTimeout(timer.current);
    // Allow the pointer to cross the gap into the reading window.
    clearTimeout(closing.current);
    closing.current = setTimeout(() => cancel(), 320);
  }
  function show() {
    clearTimeout(closing.current); clearTimeout(timer.current);
    if (isAndroid || !text || active.current) return;
    const fire = () => {
      if (!anchor.current) return;
      window.dispatchEvent(new Event("clipstash-preview-replaced"));
      beforePreview();
      active.current = true;
      const request = ++generation.current;
      const rect = anchor.current.getBoundingClientRect();
      const browserFallback = () => {
        if (generation.current !== request) return;
        const height = Math.min(TEXT_READER_SIZE.height, window.innerHeight - 24);
        const width = Math.min(TEXT_READER_SIZE.width, window.innerWidth - 24);
        setFallback({ width, height, left: Math.max(12, Math.min(rect.right + 8, window.innerWidth - width - 12)), top: Math.max(12, Math.min(rect.top, window.innerHeight - height - 12)) });
      };
      if (!("__TAURI_INTERNALS__" in window)) { browserFallback(); return; }
      void showDesktopPreview({ filename: title, text }, (w, h) => position(rect, w, h)).catch(browserFallback);
    };
    if (document.visibilityState === "hidden") fire();
    else timer.current = setTimeout(fire, Math.max(0, delay * 1000));
  }
  useEffect(() => {
    let disposed = false;
    let unlisten = () => {};
    if (!isAndroid && "__TAURI_INTERNALS__" in window) {
      void import("@tauri-apps/api/event").then(({ listen }) => listen<{ hovered?: boolean; close?: boolean }>("clipstash-text-reader", ({ payload }) => {
        if (!active.current) return;
        if (payload.close) cancel();
        else if (payload.hovered) clearTimeout(closing.current);
        else leave();
      })).then((stop) => { if (disposed) stop(); else unlisten = stop; }).catch(() => undefined);
    }
    const replace = () => cancel(false);
    const scroll = (event: Event) => {
      if (reader.current?.contains(event.target as Node)) return;
      cancel();
    };
    const keydown = (event: KeyboardEvent) => { if (event.key === "Escape") cancel(); };
    window.addEventListener("scroll", scroll, true);
    window.addEventListener("keydown", keydown);
    window.addEventListener("clipstash-preview-replaced", replace);
    return () => {
      disposed = true; unlisten();
      window.removeEventListener("scroll", scroll, true);
      window.removeEventListener("keydown", keydown);
      window.removeEventListener("clipstash-preview-replaced", replace);
      clearTimeout(timer.current); clearTimeout(closing.current);
      if (active.current) void hideDesktopPreview().catch(() => undefined);
      active.current = false;
      generation.current++;
    };
  }, [isAndroid, text]);
  useEffect(() => {
    if (fallback && reader.current) return mountTextReader(reader.current, text, title, () => cancel());
  }, [fallback, text, title]);
  return <>
    <button {...props} ref={anchor} onMouseEnter={show} onMouseLeave={leave} onFocus={show} onBlur={leave} />
    {fallback && createPortal(<div ref={reader} className="text-preview-fallback" style={fallback} onMouseEnter={() => clearTimeout(closing.current)} onMouseLeave={leave} />, document.body)}
  </>;
}
