import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { TextPreviewButton } from "../src/TextPreviewButton";

vi.mock("../src/desktopPreview", () => ({ showDesktopPreview: vi.fn().mockResolvedValue(undefined), hideDesktopPreview: vi.fn().mockResolvedValue(undefined) }));
const position = () => ({ left: 12, top: 12, width: 520, height: 803 });
const props = { text: "第一行\n第二行", title: "消息 #1", isAndroid: false, delay: .8, position, beforePreview: vi.fn() };
beforeEach(() => { vi.useFakeTimers(); Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" }); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); });
it("waits for hover delay, preserves copy, and cancels the paper on Escape", () => {
  const copy = vi.fn(); render(<TextPreviewButton {...props} onClick={copy}>消息正文</TextPreviewButton>);
  const button = screen.getByRole("button", { name: "消息正文" });
  fireEvent.mouseEnter(button); act(() => vi.advanceTimersByTime(799));
  expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
  act(() => vi.advanceTimersByTime(1)); expect(screen.getByLabelText("文本阅读预览")).toBeTruthy();
  expect((document.querySelector(".text-preview-fallback") as HTMLElement).style.width).toBe("360px");
  expect((document.querySelector(".text-preview-fallback") as HTMLElement).style.height).toBe("500px");
  fireEvent.click(button); expect(copy).toHaveBeenCalledOnce();
  fireEvent.keyDown(window, { key: "Escape" }); expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
});
it("cancels pending hover on scroll and mouse leave", () => {
  render(<TextPreviewButton {...props}>消息正文</TextPreviewButton>);
  const button = screen.getByRole("button", { name: "消息正文" });
  fireEvent.mouseEnter(button); fireEvent.mouseLeave(button); act(() => vi.advanceTimersByTime(1000));
  expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
  fireEvent.mouseEnter(button); fireEvent.scroll(window); act(() => vi.advanceTimersByTime(1000));
  expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
});
it("keeps the reader open while crossing into the paper and closes it on replacement", () => {
  render(<TextPreviewButton {...props}>消息正文</TextPreviewButton>);
  const button = screen.getByRole("button", { name: "消息正文" });
  fireEvent.mouseEnter(button); act(() => vi.advanceTimersByTime(800)); fireEvent.mouseLeave(button);
  fireEvent.mouseEnter(document.querySelector(".text-preview-fallback")!); act(() => vi.advanceTimersByTime(400));
  expect(screen.getByLabelText("文本阅读预览")).toBeTruthy();
  act(() => window.dispatchEvent(new Event("clipstash-preview-replaced")));
  expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
});
it("does not start desktop hover on Android", () => {
  render(<TextPreviewButton {...props} isAndroid>消息正文</TextPreviewButton>);
  fireEvent.mouseEnter(screen.getByRole("button", { name: "消息正文" })); act(() => vi.advanceTimersByTime(2000));
  expect(screen.queryByLabelText("文本阅读预览")).toBeNull();
});
