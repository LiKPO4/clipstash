# 阶段 2/3 真实 UI 验收清单

## 目标

- 补齐浏览器 mock 测试之外的应用级点击验收。
- 验证 Tauri WebView 中的真实按钮、弹层、文件选择、剪贴板和窗口选择流程。
- 保持旧 `clipstash.db`、旧 `images/` 和旧 Python 版兼容。

## 当前基线

- 旧库路径：`C:\Users\Administrator\AppData\Roaming\ClipStash\clipstash.db`
- 旧图片目录：`C:\Users\Administrator\AppData\Roaming\ClipStash\images`
- 当前审计基线：`normal=11 archived=103 total=114 max_id=114 joined_images=107 orphan_images=0`
- 测试主消息：`#114 archived=0 archived_at=None`，文字 52 字符，1 张图片。

## 验收总表

| 范围 | 项目 | 状态 | 证据 |
| --- | --- | --- | --- |
| 阶段 2 | 新增纯图片 | 已通过 | UI 新增 `#115`，SQLite 验证 `text_content=None` 和 1 张图片；UI 删除后旧库回基线。 |
| 阶段 2 | 新增图文 | 已通过 | UI 新增 `#116`，SQLite 验证文字和 1 张图片；UI 删除后旧库回基线。 |
| 阶段 2 | 编辑文字 | 已通过 | UI 新增并编辑 `#117`，SQLite 验证 `text_content` 更新；UI 删除后旧库回基线。 |
| 阶段 2 | 替换图片 | 已通过 | UI 新增 `#118` 后替换图片，SQLite 验证新图片关联，旧图片进入 `images.bak-*`；UI 删除后旧库回基线。 |
| 阶段 2 | 删除清理 | 已通过 | `#115`、`#116`、`#117`、`#118` 均通过 UI 删除，`messages` 和 `message_images` 均清零。 |
| 阶段 2/3 | 归档/恢复 | 已通过 | UI 归档 `#114` 后计数 `normal=10 archived=104`；UI 恢复后回到 `normal=11 archived=103`。 |
| 阶段 3 | 文字复制 | 已通过 | UI 点击 `#114` 复制文字，旧 Python 剪贴板读取 52 字符。 |
| 阶段 3 | 图片复制 | 已通过 | UI 点击 `#114` 图片复制，旧 Python 剪贴板读取 `1x1` 图片。 |
| 阶段 3 | 准备导入 | 已通过 | UI 点击 `#114` 准备导入，剪贴板读回 52 字符文本。 |
| 阶段 3 | 查看队列/复制队列项 | 已通过 | `#114` 队列显示 2 项；第 1 项文本和第 2 项图片均可复制并读回。 |
| 阶段 3 | 目标窗口刷新/选择/校验 | 已通过 | 临时 WinForms 目标窗口可被枚举、选择并校验通过。 |
| 阶段 3 | 单项粘贴 | 已通过 | 文本项粘贴到临时目标窗口，目标文件读回 52 字符文本。 |
| 阶段 3 | 整队列粘贴 | 已通过 | 整队列粘贴到临时目标窗口，目标读回文字和 `1x1` 图片。 |
| 阶段 3 | 整队列粘贴后归档 | 已通过 | 勾选归档后 `#114` 短暂归档，随后恢复并审计回基线。 |

## 关键备份

- 新增纯图片：`clipstash.db.bak-20260608-210945`
- 删除纯图片测试消息：`clipstash.db.bak-20260608-211133`
- 新增图文：`clipstash.db.bak-20260608-212900`
- 删除图文测试消息：`clipstash.db.bak-20260608-212931`
- 新增编辑测试消息：`clipstash.db.bak-20260608-213243`
- 编辑文字：`clipstash.db.bak-20260608-213312`
- 删除编辑测试消息：`clipstash.db.bak-20260608-213349`
- 新增替换图片测试消息：`clipstash.db.bak-20260608-213721`
- 替换图片：`clipstash.db.bak-20260608-213758`，图片备份 `images.bak-20260608-213758`
- 删除替换图片测试消息：`clipstash.db.bak-20260608-213831`
- UI 归档 `#114`：`clipstash.db.bak-20260608-214158`
- UI 恢复 `#114`：`clipstash.db.bak-20260608-214231`
- 带归档整队列粘贴：`clipstash.db.bak-20260608-210454`
- 带归档整队列粘贴后恢复：`clipstash.db.bak-20260608-210532`

## 验收规则

- 每次写 DB 前记录基线，写入后记录备份路径。
- 每次写图片后验证 `message_images` 和旧 `images/` 文件。
- 每次替换图片后验证旧图片进入 `images.bak-*`。
- 每次删除测试消息后验证 `messages` 和 `message_images` 对应行均为 0。
- 每次验收后运行 `npm run verify:legacy-readonly`，确认旧库回到当前基线。
- 不对历史真实消息执行不可恢复删除；写库验收优先创建临时消息再清理。

## 剩余风险

- 内置浏览器截图验收曾遇到 `net::ERR_BLOCKED_BY_CLIENT`，当前主要依赖 Tauri WebView CDP、SQLite、文件系统和剪贴板证据。
- 外部窗口粘贴验收依赖临时 WinForms 目标窗口，若目标应用自身焦点行为不同，仍需单独复测。
- 旧库写入能力已经验证，但完整迁移仍需后续规划新数据模型、导入/导出策略和最终切换策略。

## 下一步

- 开始下一阶段迁移规划：明确新库是否继续直读旧库、何时引入新 schema、如何双写/迁移、如何回滚。
- 在进入下一阶段前，保留当前基线：`normal=11 archived=103 total=114 max_id=114 joined_images=107 orphan_images=0`。
