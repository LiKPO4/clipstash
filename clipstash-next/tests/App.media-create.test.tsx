import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../src/App";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

const stats = {
  data_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash",
  db_path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db",
  images_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images",
  db_exists: true,
  images_dir_exists: true,
  normal_count: 11,
  archived_count: 103,
  total_count: 114,
};

const emptyPage = {
  view: "normal",
  sort: "newest",
  offset: 0,
  limit: 30,
  total_count: 0,
  has_more: false,
  messages: [],
};

const createResult = {
  backup: {
    source_path: stats.db_path,
    backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-170000",
    bytes_copied: 61440,
  },
  message: {
    id: 200,
    text_content: null,
    created_at: "2026-06-08 17:00:00",
    archived: false,
    archived_at: null,
    images: [
      {
        id: 301,
        filename: "clipstash-next-test.png",
        path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\clipstash-next-test.png",
        exists: true,
      },
    ],
  },
};

describe("media create form", () => {
  beforeEach(() => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") return Promise.resolve(emptyPage);
      if (command === "create_legacy_image_message") return Promise.resolve(createResult);
      if (command === "create_legacy_mixed_message") {
        return Promise.resolve({
          ...createResult,
          message: {
            ...createResult.message,
            text_content: "配套文字",
          },
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    invokeMock.mockReset();
  });

  it("requires both selected images and explicit write confirmation", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await screen.findByRole("region", { name: "新增图片或图文消息" });
    const fileInput = within(panel).getByLabelText("图片");
    const confirm = within(panel).getByLabelText(
      "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
    );
    const submit = within(panel).getByRole("button", { name: "新增并备份" });

    expect((submit as HTMLButtonElement).disabled).toBe(true);

    await user.upload(
      fileInput,
      new File([new Uint8Array([1, 2, 3])], "pixel.png", { type: "image/png" }),
    );

    expect((submit as HTMLButtonElement).disabled).toBe(true);

    await user.click(confirm);

    await waitFor(() => {
      expect((submit as HTMLButtonElement).disabled).toBe(false);
    });
  });

  it("creates an image-only message and refreshes legacy data after success", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await screen.findByRole("region", { name: "新增图片或图文消息" });
    await user.upload(
      within(panel).getByLabelText("图片"),
      new File([new Uint8Array([1, 2, 3])], "pixel.png", { type: "image/png" }),
    );
    await user.click(
      within(panel).getByLabelText(
        "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
      ),
    );
    await user.click(within(panel).getByRole("button", { name: "新增并备份" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_image_message", {
        imagesData: [[1, 2, 3]],
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "create_legacy_mixed_message",
      expect.anything(),
    );
    expect(await within(panel).findByText("已写入 #200")).toBeTruthy();
    expect(within(panel).getByText(createResult.backup.backup_path)).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("creates a mixed text and image message when text is present", async () => {
    const user = userEvent.setup();
    render(<App />);

    const panel = await screen.findByRole("region", { name: "新增图片或图文消息" });
    await user.type(within(panel).getByLabelText("配套文字"), " 配套文字 ");
    await user.upload(
      within(panel).getByLabelText("图片"),
      new File([new Uint8Array([4, 5])], "pixel.png", { type: "image/png" }),
    );
    await user.click(
      within(panel).getByLabelText(
        "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
      ),
    );
    await user.click(within(panel).getByRole("button", { name: "新增并备份" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_legacy_mixed_message", {
        textContent: "配套文字",
        imagesData: [[4, 5]],
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "create_legacy_image_message",
      expect.anything(),
    );
    expect(await within(panel).findByText("已写入 #200")).toBeTruthy();
  });
});

function commandCallCount(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command).length;
}
