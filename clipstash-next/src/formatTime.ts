/**
 * 数据库中的时间字符串（如 "2026-08-05 07:07:59"）统一按 UTC 存储，
 * 展示时转换为本机时区，与旧版 Python 端 `_format_local_time` 行为一致。
 */
export const formatLocalTime = (utcStr: string | null | undefined): string => {
  if (!utcStr) {
    return "";
  }
  const normalized = String(utcStr).split(".")[0].trim().replace(" ", "T");
  const date = new Date(`${normalized}Z`);
  if (Number.isNaN(date.getTime())) {
    return String(utcStr);
  }
  const pad = (value: number) => String(value).padStart(2, "0");
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  );
};
