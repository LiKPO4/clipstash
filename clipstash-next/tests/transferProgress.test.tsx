import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DataTransferProgress, invokeDataTransfer } from "../src/transferProgress";
import { exportNormalDataZip, importDataZipFromPath } from "../src/api/legacy";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
  Channel: class { onmessage = (_value: unknown) => {}; },
}));
afterEach(() => { cleanup(); invokeMock.mockReset(); });
function deferred() {
  let resolve!: (value: unknown) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
}

it("shows stage and actual bytes, then ignores callbacks after completion", async () => {
  const result = deferred();
  invokeMock.mockReturnValue(result.promise);
  render(<DataTransferProgress />);
  let task!: ReturnType<typeof exportNormalDataZip>;
  act(() => { task = exportNormalDataZip(); });
  expect(screen.getByText("导出：准备数据")).toBeTruthy();
  expect(screen.getByRole("progressbar").hasAttribute("value")).toBe(false);
  const channel = invokeMock.mock.calls[0][1].progress;
  const oldCallback = channel.onmessage;
  act(() => channel.onmessage({ phase: "export_write", completed_bytes: 1048576, total_bytes: 2097152 }));
  expect(screen.getByText("导出：写入压缩包")).toBeTruthy();
  expect(screen.getByText("已处理 1.0 MiB / 2.0 MiB")).toBeTruthy();
  await act(async () => { result.resolve({ path: "done.zip" }); await task; });
  act(() => oldCallback({ phase: "commit", completed_bytes: 0, total_bytes: null }));
  expect(screen.queryByLabelText("数据处理进度")).toBeNull();
});

it("failure clears only its own task and preserves concurrent progress", async () => {
  const first = deferred(), second = deferred();
  invokeMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  render(<DataTransferProgress />);
  let one!: Promise<unknown>, two!: Promise<unknown>;
  act(() => {
    one = importDataZipFromPath("data.zip").catch((error) => error);
    two = exportNormalDataZip();
  });
  expect(screen.getAllByRole("progressbar")).toHaveLength(2);
  const error = new Error("bad package");
  await act(async () => { first.reject(error); expect(await one).toBe(error); });
  expect(screen.getAllByRole("progressbar")).toHaveLength(1);
  expect(screen.getByText("导出：准备数据")).toBeTruthy();
  await act(async () => { second.resolve({}); await two; });
  expect(screen.queryByRole("progressbar")).toBeNull();
});

it("retains in-flight progress across view remount and removes it at settlement", async () => {
  const result = deferred(); invokeMock.mockReturnValue(result.promise);
  const view = render(<DataTransferProgress />);
  let task!: Promise<unknown>;
  act(() => { task = invokeDataTransfer("preview_data_zip"); });
  view.unmount();
  const channel = invokeMock.mock.calls[0][1].progress;
  channel.onmessage({ phase: "preview", completed_bytes: 4096, total_bytes: null });
  render(<DataTransferProgress />);
  expect(screen.getByText("导入预览：校验数据包")).toBeTruthy();
  await act(async () => { result.resolve({}); await task; });
  expect(screen.queryByLabelText("数据处理进度")).toBeNull();
});
