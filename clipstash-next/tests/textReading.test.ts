import { describe, expect, it } from "vitest";
import { mountTextReader, paginateText } from "../src/textReading";

describe("paper text pagination", () => {
  it("preserves paragraphs, long URLs, whitespace and Unicode across every page", () => {
    const text = "标题\n\n  保留缩进 🧩😀\nhttps://example.com/" + "long-path-".repeat(90) + "\n最后一行。";
    const pages = paginateText(text, (value) => Array.from(value).length <= 70);
    expect(pages.length).toBeGreaterThan(2);
    expect(pages.join("")).toBe(text);
    expect(pages.every((p) => Array.from(p).length <= 70)).toBe(true);
    expect(pages.some((p) => /[\uD800-\uDBFF]$/.test(p))).toBe(false);
  });
  it("bounds layout probes for very long messages and still preserves every character", () => {
    const text = "长内容".repeat(20_000);
    let largest = 0;
    const pages = paginateText(text, (candidate) => { largest = Math.max(largest, candidate.length); return candidate.length <= 1200; });
    expect(pages.join("")).toBe(text);
    expect(largest).toBeLessThanOrEqual(16_384);
  });
  it("returns a stable empty page and makes progress even in a very small viewport", () => {
    expect(paginateText("", () => true)).toEqual([""]);
    expect(paginateText("😀字", () => false)).toEqual(["😀", "字"]);
  });
  it("renders message markup as inert text and removes the reader on dispose", () => {
    const host = document.createElement("div"); document.body.append(host);
    const text = '<img src=x onerror="alert(1)">\n第二行';
    const dispose = mountTextReader(host, text, "<b>消息</b>", () => {});
    expect(host.querySelector("article")!.textContent).toBe(text);
    expect(host.querySelector("img")).toBeNull();
    expect(host.querySelector("b")).toBeNull();
    dispose(); expect(host.children.length).toBe(0); host.remove();
  });
});
