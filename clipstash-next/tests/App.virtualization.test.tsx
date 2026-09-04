import { createRef, type ComponentProps } from "react";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MessageList } from "../src/App";
import type { LegacyMessage } from "../src/api/types";

const { formatMock } = vi.hoisted(() => ({ formatMock: vi.fn(() => "时间") }));
vi.mock("../src/formatTime", () => ({ formatLocalTime: formatMock }));

type Props = ComponentProps<typeof MessageList>;
function messages(count: number): LegacyMessage[] {
  return Array.from({ length: count }, (_, index) => ({
    id: index + 1, text_content: `消息 ${index + 1}`, created_at: "2026-09-04",
    archived: false, archived_at: null, images: [],
  }));
}
function props(rows: LegacyMessage[]): Props {
  return { messages: rows, listRef: createRef<HTMLElement>(), archivingMessageId: null,
    importingMessageId: null, expandedImageMessageIds: [], hasMore: false, loadingMore: false,
    isAndroid: false, showExternalImport: false, scrollLines: 1, previewDelaySeconds: 0.8,
    onArchive: vi.fn(), onCopyText: vi.fn(), onDelete: vi.fn(), onEdit: vi.fn(),
    onMessageDoubleClick: vi.fn(), onLoadMore: vi.fn(), onOpenImportQueue: vi.fn(),
    onBlankDoubleClick: vi.fn(), onToggleImages: vi.fn(), onPreview: vi.fn(),
  };
}

let resizeCallbacks: Array<{ callback: ResizeObserverCallback; targets: Set<Element> }>;
beforeEach(() => {
  formatMock.mockClear();
  resizeCallbacks = [];
  vi.stubGlobal("ResizeObserver", class {
    record: typeof resizeCallbacks[number];
    constructor(callback: ResizeObserverCallback) {
      this.record = { callback, targets: new Set() };
      resizeCallbacks.push(this.record);
    }
    observe(target: Element) { this.record.targets.add(target); }
    unobserve(target: Element) { this.record.targets.delete(target); }
    disconnect() { this.record.targets.clear(); }
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
    return this.classList.contains("message-list") ? 600 : Number(this.dataset.height ?? 180);
  });
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(370);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    return { width: 370, height: this.offsetHeight, top: 0, left: 0, right: 370, bottom: this.offsetHeight, x: 0, y: 0, toJSON() {} };
  });
  vi.stubGlobal("scrollTo", vi.fn());
  Object.defineProperty(HTMLElement.prototype, "scrollTo", { configurable: true, value: function (options: ScrollToOptions) { this.scrollTop = options.top ?? this.scrollTop; } });
});
afterEach(() => {
  cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals();
  delete (HTMLElement.prototype as unknown as { scrollTo?: unknown }).scrollTo;
});

describe("bounded message rendering", () => {
  it.each([100, 1_000, 10_000])("renders a viewport rather than all %i rows", (count) => {
    const { container } = render(<MessageList {...props(messages(count))} />);
    expect(container.querySelectorAll(".message-card").length).toBeGreaterThan(0);
    expect(container.querySelectorAll(".message-card").length).toBeLessThan(25);
    expect(screen.queryByText(`消息 ${count}`)).toBeNull();
  });

  it("scrolls to distant rows and keeps the focused row mounted", () => {
    const p = props(messages(10_000));
    const { container } = render(<MessageList {...p} />);
    const list = p.listRef.current!;
    const first = within(container.querySelector("article")!).getByRole("button", { name: "编辑" });
    act(() => first.focus());
    fireEvent.scroll(list, { target: { scrollTop: 190 * 5_000 } });
    expect(screen.getByText("消息 5001")).toBeTruthy();
    expect(document.activeElement).toBe(first);
    expect(container.querySelectorAll(".message-card").length).toBeLessThan(25);
    act(() => first.blur());
    expect(screen.queryByText("消息 1")).toBeNull();
  });

  it("skips unrelated card renders while event handlers use the latest committed props", () => {
    const p = props(messages(30));
    const { rerender } = render(<MessageList {...p} />);
    expect(formatMock).toHaveBeenCalledTimes(30);
    const currentEdit = vi.fn();
    rerender(<MessageList {...p} onEdit={currentEdit} onArchive={vi.fn()} />);
    expect(formatMock).toHaveBeenCalledTimes(30);
    fireEvent.click(screen.getAllByRole("button", { name: "编辑" })[0]);
    expect(currentEdit).toHaveBeenCalledWith(p.messages[0]);
    expect(p.onEdit).not.toHaveBeenCalled();
    rerender(<MessageList {...p} expandedImageMessageIds={[2]} />);
    expect(formatMock).toHaveBeenCalledTimes(31);
  });

  it("remeasures a row after expansion and retains its height after scrolling away", () => {
    const p = props(messages(1_000));
    const { container } = render(<MessageList {...p} />);
    const space = container.querySelector<HTMLElement>(".message-virtual-space")!;
    const initialHeight = parseFloat(space.style.height);
    const first = container.querySelector<HTMLElement>("[data-index='0']")!;
    first.dataset.height = "500";
    act(() => {
      resizeCallbacks.forEach(({ callback, targets }) => {
        if (targets.has(first)) callback([{ target: first, borderBoxSize: [{ blockSize: 500, inlineSize: 370 }] } as unknown as ResizeObserverEntry], {} as ResizeObserver);
      });
    });
    expect(parseFloat(space.style.height)).toBe(initialHeight + 320);
    fireEvent.scroll(p.listRef.current!, { target: { scrollTop: 190 * 500 } });
    expect(parseFloat(space.style.height)).toBe(initialHeight + 320);
  });

  it("wheel pagination sees the latest page state and is disabled while loading", () => {
    const p = props(messages(2));
    const { rerender } = render(<MessageList {...p} scrollLines={2} />);
    fireEvent.wheel(p.listRef.current!, { deltaY: 1 });
    expect(p.onLoadMore).not.toHaveBeenCalled();
    rerender(<MessageList {...p} scrollLines={2} hasMore />);
    fireEvent.wheel(p.listRef.current!, { deltaY: 1 });
    expect(p.onLoadMore).toHaveBeenCalledTimes(1);
    rerender(<MessageList {...p} scrollLines={2} hasMore loadingMore />);
    fireEvent.wheel(p.listRef.current!, { deltaY: 1 });
    expect(p.onLoadMore).toHaveBeenCalledTimes(1);
  });

  it("preserves the scroll offset when pagination enables virtualization and appends rows", () => {
    const rows = messages(120);
    const p = props(rows.slice(0, 60));
    const { rerender } = render(<MessageList {...p} />);
    p.listRef.current!.scrollTop = 190 * 30;
    rerender(<MessageList {...p} messages={rows.slice(0, 90)} />);
    expect(p.listRef.current!.scrollTop).toBe(190 * 30);
    expect(screen.getByText("消息 31")).toBeTruthy();
    rerender(<MessageList {...p} messages={rows} />);
    expect(p.listRef.current!.scrollTop).toBe(190 * 30);
    expect(screen.getByText("消息 31")).toBeTruthy();
  });
});
