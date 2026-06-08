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
  normal_count: 1,
  archived_count: 0,
  total_count: 1,
};

const message = {
  id: 10,
  text_content: "旧文字",
  created_at: "2026-06-08 17:10:00",
  archived: false,
  archived_at: null,
  images: [
    {
      id: 20,
      filename: "old.png",
      path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\old.png",
      exists: true,
    },
  ],
};

const page = {
  view: "normal",
  sort: "newest",
  offset: 0,
  limit: 30,
  total_count: 1,
  has_more: false,
  messages: [message],
};

let listedMessages = [message];
let writeTextMock: ReturnType<typeof vi.fn>;

const updateResult = {
  backup: {
    source_path: stats.db_path,
    backup_path:
      "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\clipstash.db.bak-20260608-171000",
    bytes_copied: 61440,
  },
  message: {
    ...message,
    text_content: "新文字",
  },
};

const replaceResult = {
  backup: updateResult.backup,
  image_backup: {
    backup_dir: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images.bak-20260608-171000",
    filenames: ["old.png"],
  },
  message: {
    ...message,
    images: [
      {
        id: 21,
        filename: "new.png",
        path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\new.png",
        exists: true,
      },
    ],
  },
};

const deleteResult = {
  backup: updateResult.backup,
  image_backup: replaceResult.image_backup,
  message,
};

const archiveResult = {
  backup: updateResult.backup,
  message: {
    ...message,
    archived: true,
    archived_at: "2026-06-08 17:30:00",
  },
};

const restoreResult = {
  backup: updateResult.backup,
  message: {
    ...message,
    archived: false,
    archived_at: null,
  },
};

const copyImageResult = {
  filename: "old.png",
  path: "C:\\Users\\Administrator\\AppData\\Roaming\\ClipStash\\images\\old.png",
  width: 12,
  height: 8,
};

const importStageResult = {
  message_id: 10,
  staged_kind: "text",
  text_length: 3,
  image_count: 1,
  first_image_filename: "old.png",
  copied_image: null,
};

const importQueuePreview = {
  message_id: 10,
  item_count: 2,
  text_length: 3,
  image_count: 1,
  skipped_missing_image_count: 0,
  items: [
    {
      kind: "text",
      text: "旧文字",
      text_length: 3,
      image: null,
    },
    {
      kind: "image",
      text: null,
      text_length: 0,
      image: message.images[0],
    },
  ],
};

const importQueueCopyResult = {
  message_id: 10,
  item_index: 0,
  staged_kind: "text",
  text_length: 3,
  image_filename: null,
  copied_image: null,
};

const externalWindowTargets = [
  {
    hwnd: 1001,
    process_id: 2001,
    title: "记事本",
  },
  {
    hwnd: 1002,
    process_id: 2002,
    title: "浏览器输入框",
  },
];

describe("edit and delete guarded actions", () => {
  beforeEach(() => {
    listedMessages = [message];
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      if (command === "get_legacy_stats") return Promise.resolve(stats);
      if (command === "list_legacy_messages") {
        return Promise.resolve({
          ...page,
          total_count: listedMessages.length,
          messages: listedMessages,
        });
      }
      if (command === "update_legacy_message_text") return Promise.resolve(updateResult);
      if (command === "replace_legacy_message_images") return Promise.resolve(replaceResult);
      if (command === "delete_legacy_message") return Promise.resolve(deleteResult);
      if (command === "set_legacy_message_archived") {
        return Promise.resolve(args?.archived ? archiveResult : restoreResult);
      }
      if (command === "copy_legacy_image_to_clipboard") {
        return Promise.resolve(copyImageResult);
      }
      if (command === "stage_legacy_message_import_to_clipboard") {
        return Promise.resolve(importStageResult);
      }
      if (command === "preview_legacy_message_import_queue") {
        return Promise.resolve(importQueuePreview);
      }
      if (command === "copy_legacy_message_import_queue_item_to_clipboard") {
        return Promise.resolve(importQueueCopyResult);
      }
      if (command === "list_external_window_targets") {
        return Promise.resolve(externalWindowTargets);
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  afterEach(() => {
    cleanup();
    invokeMock.mockReset();
    vi.restoreAllMocks();
  });

  it("updates message text only after explicit confirmation", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }));

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    const save = within(dialog).getByRole("button", { name: "保存并备份" });
    expect((save as HTMLButtonElement).disabled).toBe(true);

    await user.clear(within(dialog).getByLabelText("文字内容"));
    await user.type(within(dialog).getByLabelText("文字内容"), " 新文字 ");
    expect((save as HTMLButtonElement).disabled).toBe(true);

    await user.click(
      within(dialog).getByLabelText(
        "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
      ),
    );
    await user.click(save);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("update_legacy_message_text", {
        messageId: 10,
        textContent: "新文字",
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "replace_legacy_message_images",
      expect.anything(),
    );
    expect(await within(dialog).findByText("已保存 #10")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("does not allow clearing the only content from a text-only message", async () => {
    listedMessages = [
      {
        ...message,
        id: 11,
        text_content: "只有文字",
        images: [],
      },
    ];
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#11");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 11" });
    await user.clear(within(dialog).getByLabelText("文字内容"));
    await user.click(
      within(dialog).getByLabelText(
        "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
      ),
    );

    expect(
      (within(dialog).getByRole("button", { name: "保存并备份" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith(
      "update_legacy_message_text",
      expect.anything(),
    );
  });

  it("replaces message images when replacement files are selected", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "编辑" }));

    const dialog = await screen.findByRole("dialog", { name: "编辑消息 10" });
    await user.upload(
      within(dialog).getByLabelText("替换图片"),
      new File([new Uint8Array([9, 8])], "new.png", { type: "image/png" }),
    );
    await user.click(
      within(dialog).getByLabelText(
        "确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。",
      ),
    );
    await user.click(within(dialog).getByRole("button", { name: "保存并备份" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("replace_legacy_message_images", {
        messageId: 10,
        imagesData: [[9, 8]],
      });
    });
  });

  it("deletes a message only after explicit confirmation", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(within(card.closest("article") as HTMLElement).getByRole("button", { name: "删除" }));

    const dialog = await screen.findByRole("dialog", { name: "删除消息 10" });
    const submit = within(dialog).getByRole("button", { name: "删除并备份" });
    expect((submit as HTMLButtonElement).disabled).toBe(true);

    await user.click(
      within(dialog).getByLabelText("确认删除这条旧库消息，并保留自动备份用于回滚。"),
    );
    await user.click(submit);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("delete_legacy_message", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已删除 #10")).toBeTruthy();
  });

  it("archives a normal message and refreshes legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "归档" }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_legacy_message_archived", {
        messageId: 10,
        archived: true,
      });
    });
    expect(await screen.findByText("已归档 #10")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(2);
    expect(commandCallCount("list_legacy_messages")).toBe(2);
  });

  it("restores an archived message and refreshes legacy data", async () => {
    listedMessages = [
      {
        ...message,
        archived: true,
        archived_at: "2026-06-08 17:30:00",
      },
    ];
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "已归档" }));
    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", { name: "恢复" }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_legacy_message_archived", {
        messageId: 10,
        archived: false,
      });
    });
    expect(await screen.findByText("已恢复 #10")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(3);
    expect(commandCallCount("list_legacy_messages")).toBe(3);
  });

  it("copies message text without invoking a write command", async () => {
    const user = userEvent.setup();
    writeTextMock = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "复制文字",
      }),
    );

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalledWith("旧文字");
    });
    expect(await screen.findByText("已复制 #10")).toBeTruthy();
    expect(screen.getByText("3 个字符")).toBeTruthy();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "update_legacy_message_text",
      expect.anything(),
    );
    expect(invokeMock).not.toHaveBeenCalledWith(
      "set_legacy_message_archived",
      expect.anything(),
    );
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("copies a message image without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const imageGrid = await screen.findByLabelText("图片缩略图");
    await user.click(within(imageGrid).getByRole("button", { name: "复制图片" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("copy_legacy_image_to_clipboard", {
        filename: "old.png",
      });
    });
    expect(await screen.findByText("已复制图片")).toBeTruthy();
    expect(screen.getByText("old.png · 12 × 8")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("stages a message import without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "准备导入",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("stage_legacy_message_import_to_clipboard", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("已准备导入 #10")).toBeTruthy();
    expect(screen.getByText("3 个字符已进入剪贴板")).toBeTruthy();
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("previews an import queue and copies a queue item without refreshing legacy data", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "查看队列",
      }),
    );

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("preview_legacy_message_import_queue", {
        messageId: 10,
      });
    });
    expect(await screen.findByText("导入队列 #10")).toBeTruthy();
    expect(screen.getByText("2 项 · 文字 3 字符 · 图片 1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "复制第 1 项" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "copy_legacy_message_import_queue_item_to_clipboard",
        {
          messageId: 10,
          itemIndex: 0,
        },
      );
    });
    expect(await screen.findByText("已复制导入项 #10 / 1")).toBeTruthy();
    expect(screen.getAllByText("3 个字符已进入剪贴板").length).toBeGreaterThan(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });

  it("loads and selects an external target window without pasting", async () => {
    const user = userEvent.setup();
    render(<App />);

    const card = await screen.findByText("#10");
    await user.click(
      within(card.closest("article") as HTMLElement).getByRole("button", {
        name: "查看队列",
      }),
    );
    await screen.findByText("导入队列 #10");

    await user.click(screen.getByRole("button", { name: "刷新目标窗口" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("list_external_window_targets");
    });

    await user.selectOptions(screen.getByLabelText("选择目标窗口"), "1001");
    expect(screen.getByText("已选择：记事本 · hwnd 1001")).toBeTruthy();
    expect(commandCallCount("copy_legacy_message_import_queue_item_to_clipboard")).toBe(0);
    expect(commandCallCount("get_legacy_stats")).toBe(1);
    expect(commandCallCount("list_legacy_messages")).toBe(1);
  });
});

function commandCallCount(command: string) {
  return invokeMock.mock.calls.filter(([calledCommand]) => calledCommand === command).length;
}
