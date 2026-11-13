import { describe, expect, it } from "vitest";
import { formatLocalTime, messageDayGroup } from "../src/formatTime";

describe("formatLocalTime", () => {
  it("groups UTC timestamps by their real local day and marks today and yesterday", () => {
    const raw = "2026-11-13 20:30:00";
    const localDay = new Date("2026-11-13T20:30:00Z");
    const tomorrow = new Date(localDay); tomorrow.setDate(tomorrow.getDate() + 1);
    expect(messageDayGroup(raw, localDay).label).toContain("今天 · ");
    expect(messageDayGroup(raw, tomorrow).label).toContain("昨天 · ");
    expect(messageDayGroup(raw, localDay).key).toBe(`${localDay.getFullYear()}-${localDay.getMonth()}-${localDay.getDate()}`);
  });
  it("handles unknown dates and includes the year for older messages", () => {
    expect(messageDayGroup("bad").label).toBe("日期未知");
    expect(messageDayGroup("2025-01-01 12:00:00", new Date("2026-11-13T12:00:00Z")).label).toContain("2025 年");
  });
  it("将 UTC 字符串转换为本机时区", () => {
    const result = formatLocalTime("2026-08-05 07:07:59");
    const expectedDate = new Date("2026-08-05T07:07:59Z");
    const pad = (value: number) => String(value).padStart(2, "0");
    const expected =
      `${expectedDate.getFullYear()}-${pad(expectedDate.getMonth() + 1)}-${pad(expectedDate.getDate())} ` +
      `${pad(expectedDate.getHours())}:${pad(expectedDate.getMinutes())}:${pad(expectedDate.getSeconds())}`;
    expect(result).toBe(expected);
  });

  it("兼容带小数秒的格式", () => {
    expect(formatLocalTime("2026-08-05 07:07:59.123")).toBe(
      formatLocalTime("2026-08-05 07:07:59"),
    );
  });

  it("空值返回空字符串", () => {
    expect(formatLocalTime(null)).toBe("");
    expect(formatLocalTime(undefined)).toBe("");
    expect(formatLocalTime("")).toBe("");
  });

  it("无法解析时原样返回", () => {
    expect(formatLocalTime("not-a-time")).toBe("not-a-time");
  });
});
