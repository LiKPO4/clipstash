import { describe, expect, it } from "vitest";
import { formatLocalTime } from "../src/formatTime";

describe("formatLocalTime", () => {
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
