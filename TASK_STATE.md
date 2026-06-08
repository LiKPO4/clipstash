# TASK_STATE.md

## 当前目标

- 将 ClipStash 逐步从 Python + Tkinter/customtkinter 重构为 Tauri 2 + React + TypeScript + Rust + SQLite。
- 当前阶段为 MVP-0：新应用只读旧 `clipstash.db`，显示普通消息数、已归档消息数、总消息数。

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

## 未完成

- 尚未运行 `npm run tauri dev` 并人工查看真实 Tauri WebView 首屏。
- 尚未实现消息列表、图片读取、复制、编辑、归档等后续阶段功能。

## 阻塞

- 当前无阻塞。

## 关键文件

- `clipstash-next/src-tauri/src/legacy_data.rs`
- `clipstash-next/src-tauri/src/lib.rs`
- `clipstash-next/src/App.tsx`
- `clipstash-next/src/App.css`
- `clipstash-next/src/api/legacy.ts`
- `clipstash-next/src/api/types.ts`

## 下一步

- 进入阶段 1：实现只读消息列表，读取 `messages` 与 `message_images`，展示普通/已归档列表和图片文件状态。
- 在进入写入阶段前继续保持只读，不改旧 DB、不改旧 `images/`。
