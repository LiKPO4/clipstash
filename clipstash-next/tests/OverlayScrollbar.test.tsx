import { createRef } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { OverlayScrollbar } from "../src/OverlayScrollbar";

afterEach(cleanup);

function setup(contentHeight: number, floating = false) {
  const targetRef = createRef<HTMLDivElement>();
  render(<><div ref={(node) => {
    targetRef.current = node;
    if (node) Object.defineProperties(node, {
      clientHeight: { configurable: true, value: 600 },
      scrollHeight: { configurable: true, writable: true, value: contentHeight },
    });
  }} /><OverlayScrollbar targetRef={targetRef} floating={floating} label="测试滚动条" /></>);
  return targetRef.current!;
}

describe("overlay scrollbar", () => {
  it("can align to the dialog edge while keeping the textarea as its scroll target", () => {
    const targetRef = createRef<HTMLTextAreaElement>();
    const edgeRef = createRef<HTMLElement>();
    render(<section ref={(node) => {
      edgeRef.current = node;
      if (node) node.getBoundingClientRect = () => ({ right: 350 }) as DOMRect;
    }}><textarea ref={(node) => {
      targetRef.current = node;
      if (node) Object.defineProperties(node, {
        clientHeight: { value: 400 }, scrollHeight: { value: 1200 },
      });
    }} /><OverlayScrollbar targetRef={targetRef} rightEdgeRef={edgeRef} floating /></section>);
    const bar = screen.getByRole("scrollbar");
    expect(bar.style.left).toBe("337px");
    expect(bar.style.height).toBe("400px");
    fireEvent.keyDown(bar, { key: "End" });
    expect(targetRef.current!.scrollTop).toBe(800);
  });

  it("places a floating scrollbar outside the scroll content without moving the content", () => {
    const target = setup(1800, true);
    const bar = screen.getByRole("scrollbar", { name: "测试滚动条" });
    expect(bar.parentElement).toBe(document.body);
    expect(target.parentElement).not.toBe(document.body);
    expect(bar.style.height).toBe("600px");
    expect(target.contains(bar)).toBe(false);
  });

  it("refreshes overflow after typing even when the scroll container size is unchanged", () => {
    const target = setup(600, true);
    expect(screen.queryByRole("scrollbar")).toBeNull();
    Object.defineProperty(target, "scrollHeight", { configurable: true, value: 1800 });
    fireEvent.input(target);
    expect(screen.getByRole("scrollbar").getAttribute("aria-valuemax")).toBe("1200");
  });

  it("does not show a thumb when the content fits", () => {
    setup(600);
    expect(screen.queryByRole("scrollbar")).toBeNull();
  });

  it("tracks the real scroll offset and content proportions", () => {
    const target = setup(1800);
    const bar = screen.getByRole("scrollbar");
    expect(bar.getAttribute("aria-valuemax")).toBe("1200");
    expect((bar.firstChild as HTMLElement).style.height).toBe("200px");
    target.scrollTop = 1200;
    fireEvent.scroll(target);
    expect(bar.getAttribute("aria-valuenow")).toBe("1200");
    expect((bar.firstChild as HTMLElement).style.transform).toBe("translateY(400px)");
  });

  it("supports keyboard scrolling without changing the scroll container", () => {
    const target = setup(1800);
    const bar = screen.getByRole("scrollbar");
    fireEvent.keyDown(bar, { key: "PageDown" });
    expect(target.scrollTop).toBe(600);
    fireEvent.keyDown(bar, { key: "End" });
    expect(target.scrollTop).toBe(1200);
    fireEvent.keyDown(bar, { key: "Home" });
    expect(target.scrollTop).toBe(0);
  });
});
