import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createLegacyImageMessage,
  createLegacyMixedMessage,
  createLegacyTextMessage,
  deleteLegacyMessage,
  pasteLegacyImportQueue,
  pasteLegacyImportQueueWithOptionalArchive,
  replaceLegacyMessageImages,
  setLegacyMessageArchived,
  updateLegacyMessageText,
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

  it("maps text creation arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 115 } });

    await createLegacyTextMessage("新增文字");

    expect(invokeMock).toHaveBeenCalledWith("create_legacy_text_message", {
      textContent: "新增文字",
    });
  });

  it("maps image creation arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 115 } });

    await createLegacyImageMessage([[1, 2, 3]]);

    expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
      imagesData: [[1, 2, 3]],
    });
  });

  it("maps mixed creation arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 115 } });

    await createLegacyMixedMessage("新增图文", [[1, 2, 3]]);

    expect(invokeMock).toHaveBeenCalledWith("create_legacy_mixed_message", {
      textContent: "新增图文",
      imagesData: [[1, 2, 3]],
    });
  });

  it("maps text update arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 114 } });

    await updateLegacyMessageText(114, "更新后的文字");

    expect(invokeMock).toHaveBeenCalledWith("update_legacy_message_text", {
      messageId: 114,
      textContent: "更新后的文字",
    });
  });

  it("maps image replacement arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 114 } });

    await replaceLegacyMessageImages(114, [[1, 2, 3]]);

    expect(invokeMock).toHaveBeenCalledWith("replace_legacy_message_images", {
      messageId: 114,
      imagesData: [[1, 2, 3]],
    });
  });

  it("maps delete arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 114 } });

    await deleteLegacyMessage(114);

    expect(invokeMock).toHaveBeenCalledWith("delete_legacy_message", {
      messageId: 114,
    });
  });

  it("maps archive state arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ message: { id: 114, archived: true } });

    await setLegacyMessageArchived(114, true);

    expect(invokeMock).toHaveBeenCalledWith("set_legacy_message_archived", {
      messageId: 114,
      archived: true,
    });
  });
});
