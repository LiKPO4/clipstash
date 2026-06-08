# TASK_STATE.md

## 当前目标

- 将 ClipStash 逐步从 Python + Tkinter/customtkinter 重构为 Tauri 2 + React + TypeScript + Rust + SQLite。
- 当前阶段为阶段 2 基础写入：已完成新增纯文字消息的备份写入验收，继续推进图片/图文消息写入。

## 已完成

- 已在仓库内创建 `clipstash-next/`，使用 Tauri 2 + React + TypeScript 模板。
- 已实现 Rust 侧旧数据目录定位：优先 `%APPDATA%\ClipStash`，回退 `%USERPROFILE%\ClipStash`。
- 已实现 Rust 侧只读打开旧 `clipstash.db` 并查询 `messages` 计数的 command：`get_legacy_stats`。
- 已实现 React 首屏展示总消息、普通消息、已归档消息和旧数据路径状态。
- 已运行 `npm run build`，前端 TypeScript/Vite 构建通过。
- 已安装 Rust stable minimal 工具链：`cargo 1.96.0`、`rustc 1.96.0`。
- 已运行 `cargo check`，Rust/Tauri 后端编译检查通过。
- 已运行 Rust 单元测试，临时旧库夹具计数通过。
- 已运行本机旧库只读验证，读取到普通消息 `8`、已归档消息 `103`、总消息 `111`。
- 已运行 `npm run tauri build`，Tauri release 构建通过，生成 exe/msi/nsis 产物。
- 已实现 Rust 侧 `list_legacy_messages` command，支持普通/已归档视图、最新/最早排序、offset/limit 分页。
- 已实现按 `message_images.id` 读取图片文件名，并检查旧 `images/` 中对应文件是否存在。
- 已实现 React 首屏只读消息列表：普通/已归档切换、排序切换、加载更多、文本摘要和图片文件状态。
- 已运行 Rust 单元测试覆盖消息排序、图片顺序和图片存在/缺失状态。
- 已运行本机旧库只读列表验证，普通消息总数 `8`，首批返回 `5`，`has_more=true`。
- 已用普通浏览器检查页面基础布局，1280px 和 390px 宽度均无横向溢出。
- 已实现旧图片缩略图只读展示：存在的图片通过 Tauri `asset` protocol 加载，缺失或读取失败时显示固定尺寸占位。
- 已为 Tauri `assetProtocol` 配置最小 scope：`$DATA/ClipStash/images/*` 和 `$HOME/ClipStash/images/*`。
- 已为 Rust `tauri` 依赖开启 `protocol-asset` feature，并通过 `npm run tauri build` 验证配置可打包。
- 已实现图片预览弹层：点击可读缩略图打开大图，支持遮罩关闭、关闭按钮和 Escape 关闭。
- 已运行短时 `npm run tauri dev` 启动烟测，日志显示已启动 `target\debug\clipstash-next.exe`；随后已清理开发服务和应用进程。
- 已新增 `npm run verify:legacy-readonly`，用于本机旧库只读一致性审计。
- 已运行只读一致性审计，结果：普通 `8`、归档 `103`、总计 `111`、关联图片 `105`、孤立图片 `0`。
- 审计覆盖：普通/归档数量、分页读取总数、最新/最早排序、图片数量、图片 `id` 顺序、图片文件存在状态。
- 已完成真实 Tauri WebView 视觉验收：首屏计数 `111/8/103` 和旧路径显示正确。
- 已完成列表区域视觉验收：普通列表显示 `8/8`，消息卡片和旧图片缩略图可见。
- 已完成图片预览视觉验收：点击缩略图能打开大图弹层，文件名和关闭按钮可见，图片未撑破窗口。
- 已新增阶段 2 写入安全策略文档：`clipstash-next/migration-notes/phase-2-write-safety.md`。
- 已新增 Rust DB 备份基础设施，备份文件命名为 `clipstash.db.bak-YYYYMMDD-HHMMSS`。
- 已新增临时数据库备份测试，验证备份内容一致且源 DB 未改变。
- 已为备份时间戳新增直接依赖 `chrono`，并安装 `rustfmt` 组件用于 Rust 格式化。
- 已新增 Rust 侧纯文字消息写入函数 `create_text_message_for_path`，当前仅在临时 SQLite 测试中使用，未接入 UI command。
- 已新增临时数据库写入测试，覆盖写入前备份、新增 `messages` 一行、`archived=0`、不新增 `message_images`、旧读取函数可读回、备份库保持原消息数。
- 已新增受保护的 Rust command：`create_legacy_text_message`，内部会先创建 `clipstash.db.bak-YYYYMMDD-HHMMSS` 备份，再新增纯文字消息。
- 已新增前端 API/类型封装 `createLegacyTextMessage`。
- 已新增阶段 2 写入保护测试：空文本会在备份前被拒绝，同秒备份不会覆盖既有备份，command 底层包装只在临时数据库测试中执行。
- 已新增阶段 2 前端受控写入入口：输入纯文字、确认写入旧库、提交后显示新消息 id、创建时间和备份路径。
- 已实现新增成功后刷新旧库统计和当前消息列表。
- 已新增 ignored 手动验收测试 `manual_creates_local_legacy_text_message_with_backup`，只有设置 `CLIPSTASH_NEXT_WRITE_LEGACY_TEXT` 时才会写真实旧库。
- 已执行一次真实旧库纯文字写入验收，新增消息 `id=112`，文本为 `[ClipStash Next 验收] Tauri 阶段 2 纯文字写入兼容测试 2026-06-08`。
- 已在写入前自动创建备份：`C:\Users\Administrator\AppData\Roaming\ClipStash\clipstash.db.bak-20260608-160257`，大小 `61440` 字节。
- 已用旧 Python `db.py` 验证可读取新版创建的消息，普通列表最新项为 `id=112`，图片列表为空。
- 已用 Rust 只读一致性审计验证写入后真实旧库：普通 `9`、归档 `103`、总计 `112`、关联图片 `105`、孤立图片 `0`。
- 已实现 Rust 侧临时数据库新增图片消息写入函数，会创建 `images/`、保存 `.png` 文件、插入 `messages` 与 `message_images`。
- 已新增临时图片消息测试，覆盖写入前备份、双图文件内容、`message_images.id` 顺序、旧读取函数读回图片存在状态、备份库未变化。
- 已新增空图片消息拒绝测试，确认会在备份前失败，不产生 `clipstash.db.bak-*`。
- 已新增受保护的 Rust command：`create_legacy_image_message`，内部会先备份旧 DB，再新增图片消息；当前仅暴露 command 和前端 API，未接入 UI。
- 已新增前端 API/类型封装 `createLegacyImageMessage`。
- 已新增图片写入失败清理测试，覆盖 DB 关联插入失败后事务回滚且已保存图片文件被删除，不留下孤立图片文件。
- 已新增 ignored 手动验收测试 `manual_creates_local_legacy_image_message_with_backup`，只有设置 `CLIPSTASH_NEXT_WRITE_LEGACY_IMAGE` 时才会写真实旧库和真实 `images/`。
- 已执行一次真实旧库图片写入验收，新增消息 `id=113`，图片文件为 `clipstash-next-20260608161841911-1588-0.png`。
- 已在图片写入前自动创建备份：`C:\Users\Administrator\AppData\Roaming\ClipStash\clipstash.db.bak-20260608-161841`，大小 `61440` 字节。
- 已用旧 Python `db.py` 验证可读取新版创建的图片消息，普通列表最新项为 `id=113`，图片文件存在且大小 `70` 字节。
- 已用 Rust 只读一致性审计验证写入后真实旧库：普通 `10`、归档 `103`、总计 `113`、关联图片 `106`、孤立图片 `0`。
- 已实现 Rust 侧临时数据库新增图文混合消息写入函数，复用图片文件保存、`messages.text_content` 和 `message_images` 写入路径。
- 已新增临时图文混合消息测试，覆盖写入前备份、文字 trim、双图文件内容、`message_images.id` 顺序、旧读取函数按 `(text, images)` 结构读回、备份库未变化。
- 已新增受保护的 Rust command：`create_legacy_mixed_message`，内部会先备份旧 DB，再新增图文混合消息；当前仅暴露 command 和前端 API，未接入 UI。
- 已新增前端 API/类型封装 `createLegacyMixedMessage`。
- 已新增 ignored 手动验收测试 `manual_creates_local_legacy_mixed_message_with_backup`，只有设置 `CLIPSTASH_NEXT_WRITE_LEGACY_MIXED` 时才会写真实旧库和真实 `images/`。
- 已执行一次真实旧库图文混合写入验收，新增消息 `id=114`，文本为 `[ClipStash Next 验收] Tauri 阶段 2 图文混合写入兼容测试 2026-06-08`，图片文件为 `clipstash-next-20260608163309521-1896-0.png`。
- 已在图文写入前自动创建备份：`C:\Users\Administrator\AppData\Roaming\ClipStash\clipstash.db.bak-20260608-163309`，大小 `61440` 字节。
- 已用旧 Python `db.py` 验证可读取新版创建的图文消息，普通列表最新项为 `id=114`，图片文件存在且大小 `70` 字节。
- 已用 Rust 只读一致性审计验证写入后真实旧库：普通 `11`、归档 `103`、总计 `114`、关联图片 `107`、孤立图片 `0`。
- 已为新增图片/图文消息接入前端受控入口：文件选择、可选文字、写入确认、备份路径和写入结果显示。
- 已实现前端根据文字是否为空自动调用 `createLegacyImageMessage` 或 `createLegacyMixedMessage`，成功后刷新旧库统计和当前消息列表。
- 已新增前端 mock 交互测试，覆盖图片/图文写入入口的确认门禁、纯图片/图文 command 分流、写入成功后的统计和列表刷新。

## 未完成

- 尚未实现复制、编辑、归档、恢复、导入等后续阶段功能。
- 阶段 2 尚未对前端图片/图文入口执行真实点击写入验收；当前 mock 测试不写真实旧库。
- 阶段 2 尚未完成图片/图文入口的浏览器截图验收；当前会话未暴露可用的 in-app browser 导航/截图工具。
- 阶段 2 尚未实现编辑消息文字和图片、删除消息。

## 阻塞

- 当前无阻塞。

## 关键文件

- `clipstash-next/src-tauri/src/legacy_data.rs`
- `clipstash-next/src-tauri/src/lib.rs`
- `clipstash-next/src-tauri/tauri.conf.json`
- `clipstash-next/src-tauri/Cargo.toml`
- `clipstash-next/src-tauri/Cargo.lock`
- `clipstash-next/src/App.tsx`
- `clipstash-next/src/App.css`
- `clipstash-next/src/api/legacy.ts`
- `clipstash-next/src/api/types.ts`
- `clipstash-next/package.json`
- `clipstash-next/migration-notes/phase-2-write-safety.md`

## 下一步

- 进入阶段 2 下一步：对前端图片/图文入口执行一次可回滚的真实点击写入验收，确认 UI 显示备份路径和写入结果，并验证旧 Python 读取。
