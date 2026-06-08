import { afterEach, describe, expect, it, vi } from "vitest";
import {
  pasteLegacyImportQueue,
  pasteLegacyImportQueueWithOptionalArchive,
} from "../src/api/legacy";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("legacy api command contracts", () => {
  afterEach(() => {
    invokeMock.mockReset();
  });

  it("maps queued paste arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ completed_count: 2 });

    await pasteLegacyImportQueue(114, 123456, 250);

    expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue", {
      messageId: 114,
      targetHwnd: 123456,
      delayMs: 250,
    });
  });

  it("maps optional archive queued paste arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ archive_requested: true });

    await pasteLegacyImportQueueWithOptionalArchive({
      messageId: 114,
      targetHwnd: 123456,
      delayMs: 250,
      archiveAfterSuccess: true,
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "paste_legacy_import_queue_with_optional_archive",
      {
        messageId: 114,
        targetHwnd: 123456,
        delayMs: 250,
        archiveAfterSuccess: true,
      },
    );
  });
});
