import { type RefObject, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** An overlay thumb for a native scroll container, without reserving layout width. */
export function OverlayScrollbar({ targetRef, label = "消息列表滚动条", floating = false, refreshKey, rightEdgeRef }: {
  targetRef: RefObject<HTMLElement | null>;
  label?: string;
  floating?: boolean;
  refreshKey?: string | number;
  rightEdgeRef?: RefObject<HTMLElement | null>;
}) {
  const [metrics, setMetrics] = useState({ height: 0, top: 0, maximum: 0, value: 0, x: 0, y: 0, viewport: 0 });
  const drag = useRef<{ y: number; scroll: number } | null>(null);
  useEffect(() => {
    const target = targetRef.current;
    if (!target) return;
    const measure = () => {
      const height = target.clientHeight;
      const maximum = Math.max(0, target.scrollHeight - height);
      const thumb = maximum > 0 ? Math.min(height, Math.max(28, height * height / target.scrollHeight)) : 0;
      const rect = floating ? target.getBoundingClientRect() : null;
      const edge = floating ? rightEdgeRef?.current?.getBoundingClientRect() : null;
      const next = { height: thumb, top: maximum > 0 ? target.scrollTop / maximum * (height - thumb) : 0, maximum, value: target.scrollTop,
        x: edge ? edge.right - 13 : rect ? rect.right - target.clientLeft - 9 : 0, y: rect ? rect.top + target.clientTop : 0, viewport: height };
      setMetrics((old) => Object.keys(next).every((key) => old[key as keyof typeof old] === next[key as keyof typeof next]) ? old : next);
    };
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    const observeContent = () => {
      resize?.disconnect();
      resize?.observe(target);
      if (rightEdgeRef?.current) resize?.observe(rightEdgeRef.current);
      Array.from(target.children).forEach((child) => resize?.observe(child));
      measure();
    };
    const mutation = new MutationObserver(observeContent);
    mutation.observe(target, { childList: true, subtree: true, characterData: true });
    target.addEventListener("scroll", measure, { passive: true });
    target.addEventListener("input", measure);
    window.addEventListener("resize", measure);
    if (floating) window.addEventListener("scroll", measure, true);
    observeContent();
    return () => {
      resize?.disconnect(); mutation.disconnect(); target.removeEventListener("scroll", measure);
      target.removeEventListener("input", measure); window.removeEventListener("resize", measure);
      if (floating) window.removeEventListener("scroll", measure, true);
    };
  }, [targetRef, floating, refreshKey, rightEdgeRef]);

  if (metrics.maximum <= 0) return null;
  const bar = <div className={floating ? "overlay-scrollbar overlay-scrollbar-floating" : "overlay-scrollbar"}
    style={floating ? { left: metrics.x, top: metrics.y, height: metrics.viewport, right: "auto", bottom: "auto" } : undefined}
    role="scrollbar" aria-label={label} aria-orientation="vertical"
    aria-valuemin={0} aria-valuemax={Math.round(metrics.maximum)} aria-valuenow={Math.round(metrics.value)} tabIndex={0}
    onKeyDown={(event) => {
      const target = targetRef.current;
      if (!target) return;
      const delta = event.key === "ArrowDown" ? 40 : event.key === "ArrowUp" ? -40 : event.key === "PageDown" ? target.clientHeight : event.key === "PageUp" ? -target.clientHeight : 0;
      if (delta || event.key === "Home" || event.key === "End") {
        event.preventDefault();
        target.scrollTop = event.key === "Home" ? 0 : event.key === "End" ? metrics.maximum : target.scrollTop + delta;
      }
    }}
    onPointerDown={(event) => {
      const target = targetRef.current;
      if (!target || event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      if ((event.target as HTMLElement).classList.contains("overlay-scrollbar-thumb")) {
        drag.current = { y: event.clientY, scroll: target.scrollTop };
      } else {
        const y = event.clientY - event.currentTarget.getBoundingClientRect().top;
        target.scrollTop = Math.max(0, Math.min(metrics.maximum, (y - metrics.height / 2) / (target.clientHeight - metrics.height) * metrics.maximum));
        drag.current = { y: event.clientY, scroll: target.scrollTop };
      }
    }}
    onPointerMove={(event) => {
      const target = targetRef.current;
      if (!target || !drag.current) return;
      const travel = target.clientHeight - metrics.height;
      if (travel > 0) target.scrollTop = Math.max(0, Math.min(metrics.maximum, drag.current.scroll + (event.clientY - drag.current.y) / travel * metrics.maximum));
    }}
    onPointerUp={() => { drag.current = null; }} onLostPointerCapture={() => { drag.current = null; }}
  ><div className="overlay-scrollbar-thumb" style={{ height: metrics.height, transform: `translateY(${metrics.top}px)` }} /></div>;
  return floating ? createPortal(bar, document.body) : bar;
}
