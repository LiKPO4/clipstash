# 阶段 3 导入执行器安全设计

## 目标

- 逐步替代旧 Python 版 `_on_import_message` / `_do_import_step` 导入流程。
- 保持旧数据结构兼容：只读取 `messages`、`message_images` 和旧 `images/` 文件。
- 导入执行器默认不写 DB；只有用户显式启用“导入后归档”时，才复用现有带备份的归档 command。
- 新旧版本继续可并行运行，导入动作只影响系统剪贴板和用户选定的外部输入窗口。

## 旧版行为基线

- `db.get_message(msg_id)` 读取一条消息。
- 队列顺序固定为：先文字，再按旧 DB 图片顺序加入所有存在图片。
- 执行前通过 `_focus_return_window()` 找回旧应用记录的外部窗口。
- 每一步：
  - 文字：`copy_text_to_clipboard(data)`
  - 图片：`copy_image_to_clipboard(path)`
  - 然后 `send_ctrl_v()`
- 每项之间延迟约 `250ms`，聚焦后首次延迟约 `350ms`。
- 队列结束后，如果设置开启，会自动归档消息。

## 已有 Next 基础

- `preview_legacy_message_import_queue(message_id)`：只读预检队列。
- `copy_legacy_message_import_queue_item_to_clipboard(message_id, item_index)`：复制指定队列项到系统剪贴板。
- 前端“查看队列”可展示队列并手动逐项复制。
- `set_legacy_message_archived(message_id, archived)` 已有写前 DB 备份。

## 新执行器分阶段

### 3A：手动队列

- 已实现：用户查看队列并逐项复制。
- 不发送 Ctrl+V。
- 不聚焦外部窗口。
- 不写 DB。

### 3B：单步受控粘贴

- 新增 Rust command：`paste_legacy_import_queue_item(message_id, item_index, target_window)`。
- command 内部顺序：
  1. 重新只读生成队列，校验 `item_index` 未越界。
  2. 复制该项到剪贴板。
  3. 校验目标窗口仍存在且不是 ClipStash Next 自己。
  4. 将目标窗口置前。
  5. 发送一次 Ctrl+V。
  6. 返回执行结果，不写 DB。
- 前端必须要求用户先选择/确认目标窗口；不能默认粘贴到当前前台窗口。
- 失败时必须停在当前项，不自动继续下一项。

### 3C：整队列受控粘贴

- 只在 3B 真实验收后实现。
- 每项之间保留可配置延迟，初始默认 `250ms`。
- 每一步都重新确认目标窗口仍有效。
- 任一项失败立即停止，返回已完成项数量和失败项索引。
- 队列完成前不触发归档。

### 3D：导入后归档

- 只在整队列成功后触发。
- 必须是显式开关，默认关闭。
- 复用现有 `set_legacy_message_archived(message_id, true)`，保留 DB 备份路径。
- 归档失败不能回滚外部窗口粘贴，只提示 DB 备份/错误信息。

## 窗口目标策略

- 不直接使用“当前前台窗口”作为隐式目标。
- 先实现“记录最近外部窗口”或“列出可见窗口并由用户选择”二选一。
- 窗口校验要求：
  - hwnd 非空。
  - `IsWindow(hwnd)` 为真。
  - 进程不是 ClipStash Next 自己。
  - 粘贴前再次确认窗口标题/进程仍匹配用户选择。
- 如果目标窗口失效，返回错误，不发送 Ctrl+V。

## Windows 输入策略

- 第一版仅支持 Windows。
- Rust 侧优先用 `windows` crate 调用 Win32 API：
  - `SetForegroundWindow`
  - `ShowWindow`
  - `GetForegroundWindow`
  - `keybd_event` 或更现代的 `SendInput`
- 优先选择 `SendInput`，因为它可以一次性描述 Ctrl+V 按键序列。
- 发送 Ctrl+V 前后记录目标 hwnd 和队列项信息到返回值，不写文件日志。

## 验收门槛

- 普通测试：
  - 队列越界不复制、不粘贴。
  - 空消息不复制、不粘贴。
  - 目标窗口缺失不复制、不粘贴。
- 手动验收：
  - 使用记事本作为目标窗口。
  - 图文消息 `id=114`：先粘贴文字，再粘贴图片。
  - 每次验收后运行 `npm run verify:legacy-readonly`，确认旧库计数不变。
- 自动归档验收：
  - 只允许在完整队列粘贴成功后开启。
  - 验证 DB 备份文件存在。
  - 验证旧 Python 能读取归档状态。

## 回滚

- 代码回滚使用对应 git commit。
- 剪贴板和外部窗口粘贴不可自动回滚，需要人工从目标应用撤销。
- 如果启用了导入后归档，按 DB 备份文件回滚 `clipstash.db`。
