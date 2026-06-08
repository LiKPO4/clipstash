import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createLegacyImageMessage,
  createLegacyMixedMessage,
  createLegacyTextMessage,
  deleteLegacyMessage,
  copyLegacyImageToClipboard,
  copyLegacyMessageImportQueueItemToClipboard,
  getLegacyStats,
  listExternalWindowTargets,
  listLegacyMessages,
  pasteLegacyImportQueue,
  pasteLegacyImportQueueItem,
  pasteLegacyImportQueueWithOptionalArchive,
  previewLegacyMessageImportQueue,
  replaceLegacyMessageImages,
  setLegacyMessageArchived,
  stageLegacyMessageImportToClipboard,
  updateLegacyMessageText,
  validateExternalWindowTarget,
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

  it("maps stats requests to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ total_count: 114 });

    await getLegacyStats();

    expect(invokeMock).toHaveBeenCalledWith("get_legacy_stats");
  });

  it("maps message list arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ messages: [] });

    await listLegacyMessages({
      view: "normal",
      sort: "newest",
      offset: 20,
      limit: 10,
    });

    expect(invokeMock).toHaveBeenCalledWith("list_legacy_messages", {
      view: "normal",
      sort: "newest",
      offset: 20,
      limit: 10,
    });
  });

  it("maps external window list requests to the backend command", async () => {
    invokeMock.mockResolvedValueOnce([]);

    await listExternalWindowTargets();

    expect(invokeMock).toHaveBeenCalledWith("list_external_window_targets");
  });

  it("maps external window validation arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ valid: true });

    await validateExternalWindowTarget(123456);

    expect(invokeMock).toHaveBeenCalledWith("validate_external_window_target", {
      hwnd: 123456,
    });
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

  it("maps image copy arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ filename: "clipstash-next.png" });

    await copyLegacyImageToClipboard("clipstash-next.png");

    expect(invokeMock).toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
      filename: "clipstash-next.png",
    });
  });

  it("maps import staging arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ kind: "text" });

    await stageLegacyMessageImportToClipboard(114);

    expect(invokeMock).toHaveBeenCalledWith(
      "stage_legacy_message_import_to_clipboard",
      {
        messageId: 114,
      },
    );
  });

  it("maps import queue preview arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ items: [] });

    await previewLegacyMessageImportQueue(114);

    expect(invokeMock).toHaveBeenCalledWith("preview_legacy_message_import_queue", {
      messageId: 114,
    });
  });

  it("maps import queue item copy arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ item_index: 1 });

    await copyLegacyMessageImportQueueItemToClipboard(114, 1);

    expect(invokeMock).toHaveBeenCalledWith(
      "copy_legacy_message_import_queue_item_to_clipboard",
      {
        messageId: 114,
        itemIndex: 1,
      },
    );
  });

  it("maps import queue item paste arguments to the backend command", async () => {
    invokeMock.mockResolvedValueOnce({ item_index: 1 });

    await pasteLegacyImportQueueItem(114, 1, 123456);

    expect(invokeMock).toHaveBeenCalledWith("paste_legacy_import_queue_item", {
      messageId: 114,
      itemIndex: 1,
      targetHwnd: 123456,
    });
  });
});
