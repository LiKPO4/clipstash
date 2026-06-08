# TASK_STATE.md

## 当前目标

- 将 ClipStash 逐步从 Python + Tkinter/customtkinter 重构为 Tauri 2 + React + TypeScript + Rust + SQLite。
- 当前阶段进入阶段 2 写入安全铺垫：先在临时数据库验证新增纯文字消息，不对真实旧库写入。

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
- 已新增前端 API/类型封装 `createLegacyTextMessage`，当前未接入任何 UI 控件。
- 已新增阶段 2 写入保护测试：空文本会在备份前被拒绝，同秒备份不会覆盖既有备份，command 底层包装只在临时数据库测试中执行。

## 未完成

- 尚未实现复制、编辑、归档、恢复、导入等后续阶段功能。
- 阶段 2 尚未对真实旧库执行任何写入或备份。
- 阶段 2 尚未接入前端新增消息输入 UI。

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

- 进入阶段 2 下一步：为新增纯文字消息增加前端受控入口，默认显示备份路径和写入结果；写真实旧库前先做一次可回滚的手动验收流程。
