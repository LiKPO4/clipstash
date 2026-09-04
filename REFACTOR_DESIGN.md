# ClipStash 全面重构技术设计文档

> 目标：在不破坏旧有数据结构的前提下，将 ClipStash 从 Python + Tkinter/customtkinter 重构为更现代、更稳定、更高性能、AI 编辑更友好的桌面应用。

## 1. 背景与问题判断

当前 ClipStash 已经具备完整的核心功能：

- 保存文字、图片、图文混合消息
- 图片悬浮预览
- 消息归档与恢复
- 消息排序
- 托盘驻留
- 全局快捷键呼出与导入
- GitHub Release 检查更新
- PyInstaller + Inno Setup 打包

但近期反复出现的问题集中在 UI 层：

- 多图展开/收起触发 Tk 事件重入，导致状态反复横跳。
- 图片网格销毁和重建时出现白图、残留、控件销毁异常。
- 编辑弹窗保存后偶发残壳。
- 已归档列表一次性渲染大量图片消息导致卡顿。
- Tkinter/customtkinter 控件在图片、滚动、弹窗、销毁、异步刷新叠加时行为不稳定。

这些问题不是单个函数写错，而是架构已经接近 Tkinter/customtkinter 的舒适边界。继续小修可以维持，但长期会继续消耗大量调试成本。

## 2. 重构目标

### 2.1 必须达成

- 不破坏现有用户数据。
- 新版本能直接读取旧 `clipstash.db`。
- 新版本能直接读取旧图片文件目录。
- 新旧版本至少在一段时间内可以共存测试。
- 首个 MVP 必须覆盖核心工作流：查看、复制、保存、编辑、归档、恢复、图片预览。
- 代码结构必须适合 AI 辅助开发和长期维护。

### 2.2 明确不做

- 第一阶段不重写数据库结构。
- 第一阶段不做复杂云同步。
- 第一阶段不改用户数据存放位置。
- 第一阶段不追求功能一次性全量迁移。
- 不在旧 Python 应用内继续堆新架构。

## 3. 推荐技术栈

首推：

```text
Tauri 2 + React + TypeScript + Rust + SQLite
```

### 3.1 选择理由

#### Tauri 2

- 使用系统 WebView，安装包和运行内存通常比 Electron 小。
- 后端是 Rust，适合处理文件系统、SQLite、剪贴板、全局快捷键、托盘等系统能力。
- Windows 支持成熟，适合作为桌面应用壳。
- 前后端边界清晰，便于逐步迁移。

#### React + TypeScript

- AI 编辑友好，组件、状态、类型、测试边界都清楚。
- UI 重绘、列表虚拟化、图片预览、弹窗、状态管理都比 Tkinter 更自然。
- 可复用大量成熟库，例如虚拟列表、快捷键、弹窗、Toast。

#### Rust

- 运行效率高。
- 错误类型和模块边界更严谨。
- 适合封装旧数据兼容层、图片文件访问、剪贴板和系统集成。

#### SQLite

- 保持现有数据库。
- 不引入服务器或同步复杂度。
- 可以先做兼容读取，再逐步加 schema version 和迁移。

## 4. 旧数据兼容设计

### 4.1 当前数据位置

当前 Python 版本通过 `config.py` 管理数据目录：

```text
DATA_DIR
  clipstash.db
  settings.json
  images/
```

真实用户数据已知示例：

```text
C:\Users\Administrator\AppData\Roaming\ClipStash\clipstash.db
```

新应用必须继续使用同一目录，不能默认创建新的孤立数据目录。

### 4.2 当前数据库结构

当前 `db.py` 中核心表为：

```sql
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text_content TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    archived INTEGER DEFAULT 0,
    archived_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS message_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    image_filename TEXT NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

兼容规则：

- 必须保留 `messages` 表。
- 必须保留 `message_images` 表。
- 必须保留图片文件名关联方式。
- 不得重排旧图片顺序，读取时继续 `ORDER BY id`。
- `archived=0` 表示普通消息，`archived=1` 表示已归档。
- 普通消息排序默认使用 `created_at`。
- 已归档消息排序默认使用 `COALESCE(archived_at, created_at)`。

### 4.3 数据访问策略

新应用建立 Rust 模块：

```text
src-tauri/src/db/
  mod.rs
  models.rs
  legacy.rs
  messages.rs
  images.rs
  migrations.rs
```

其中：

- `legacy.rs` 只负责旧 schema 兼容。
- `messages.rs` 提供应用级查询和写入。
- `migrations.rs` 只做非破坏性迁移。

所有迁移必须满足：

- 只增加字段或表。
- 不删除旧字段。
- 不重命名旧表。
- 不改变旧数据语义。
- 执行前备份数据库到 `clipstash.db.bak-YYYYMMDD-HHMMSS`。

## 5. 新架构总览

建议目录：

```text
clipstash-next/
  package.json
  vite.config.ts
  src/
    app/
      App.tsx
      routes.tsx
    components/
      MessageCard/
      MessageList/
      ImageGrid/
      EditorDialog/
      SettingsDialog/
      HoverPreview/
      Toolbar/
    stores/
      messageStore.ts
      settingsStore.ts
      uiStore.ts
    api/
      tauriCommands.ts
      types.ts
    styles/
      theme.css
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      commands/
        messages.rs
        clipboard.rs
        settings.rs
        update.rs
        window.rs
      db/
      images/
      clipboard/
      hotkeys/
      tray/
      updater/
      errors.rs
  tests/
  migration-notes/
```

### 5.1 前端职责

前端只负责：

- 页面布局。
- 消息列表展示。
- 图片网格展示。
- 编辑器状态。
- 设置页状态。
- 调用 Tauri commands。
- 显示 Toast、Loading、Error。

前端不直接访问文件系统和 SQLite。

### 5.2 Rust 后端职责

Rust 负责：

- 读取和写入 SQLite。
- 解析旧图片路径。
- 保存图片文件。
- 剪贴板读写。
- 全局快捷键。
- 托盘菜单。
- GitHub Release 检查。
- 自动导入逻辑。
- 数据迁移和备份。

### 5.3 IPC 边界

推荐 Tauri commands：

```text
list_messages(view, sort, cursor, limit) -> MessagePage
get_message(id) -> Message
create_message(input) -> Message
update_message(id, input) -> Message
delete_message(id) -> bool
toggle_archive(id) -> Message
copy_text(id) -> bool
copy_image(image_id) -> bool
import_message(id) -> bool
read_clipboard() -> ClipboardPayload
save_settings(settings) -> Settings
get_settings() -> Settings
check_update() -> UpdateResult
open_release_page() -> bool
```

数据类型必须集中定义：

```ts
type Message = {
  id: number;
  textContent: string | null;
  images: MessageImage[];
  createdAt: string;
  archived: boolean;
  archivedAt: string | null;
};

type MessageImage = {
  id: number;
  filename: string;
  url: string;
  width?: number;
  height?: number;
};
```

## 6. UI 设计原则

### 6.1 消息列表

必须使用虚拟列表或增量列表，不再一次性渲染所有消息。

推荐：

- 普通列表和已归档列表共享一个 `MessageList`。
- 使用分页或 cursor。
- 首屏优先加载。
- 图片懒加载。
- 缩略图使用固定尺寸画布。
- 展开状态只存在 UI store 中，不写 DB。

### 6.2 图片展示

必须避免旧版问题：

- 不在状态切换时销毁整个卡片。
- 图片网格和展开按钮分离。
- 缩略图统一固定尺寸，原图只在预览时读取。
- 图片加载失败显示明确占位，不显示空白块。
- 悬浮预览必须先计算屏幕可见区域再显示。

### 6.3 编辑器

编辑器必须是受控状态：

```text
text draft
image draft list
dirty flag
saving flag
error state
```

保存流程：

```text
点击保存
  -> 禁用保存按钮
  -> 调用 update/create command
  -> 成功后关闭弹窗
  -> 更新列表缓存
  -> Toast
  -> 失败则保持弹窗并显示错误
```

### 6.4 设置页

设置页不应该直接散落调用系统能力。保存时：

```text
编辑 UI draft
  -> 保存
  -> Rust 写 settings.json
  -> Rust 更新热键/开机启动
  -> 返回实际生效值
```

检查更新失败时：

- 显示失败原因。
- 同时显示可点击 Release 页面链接。
- 不把失败状态伪装成普通提示。

## 7. 功能迁移阶段

### 阶段 0：重构准备

目标：

- 创建 `clipstash-next`。
- 确认 Tauri 模板可运行。
- 建立旧数据目录定位。
- 建立 SQLite 只读连接。

验收：

- 新应用能启动。
- 能显示当前 DB 中消息总数。
- 不写入任何旧数据。

### 阶段 1：只读消息列表

目标：

- 读取 `messages` 和 `message_images`。
- 展示普通消息。
- 展示已归档消息。
- 展示缩略图。
- 支持排序。

验收：

- 新旧应用消息数量一致。
- 图片数量一致。
- 多图消息顺序一致。
- 归档和普通列表归属一致。

### 阶段 2：基础写入

目标：

- 新建文字消息。
- 新建图片消息。
- 新建图文混合消息。
- 编辑消息文字和图片。
- 删除消息。

验收：

- 旧版 Python 应用能读取新版创建的数据。
- 新版能读取旧版创建的数据。
- 图片文件路径和 DB 记录一致。

### 阶段 3：归档、恢复、复制、导入

目标：

- 归档/恢复。
- 复制文本。
- 复制图片。
- 导入消息到外部窗口。
- 自动导入后归档。

验收：

- 归档状态不会出现双列表同时显示。
- DB 中 `archived` 和 `archived_at` 正确。
- 导入流程失败时不吞数据。

### 阶段 4：系统能力

目标：

- 托盘。
- 全局快捷键。
- 开机启动。
- 文件拖拽。
- 更新检查。

验收：

- 快捷键可保存并重启后生效。
- 托盘菜单可呼出和退出。
- 更新失败可跳转 Release 页面。

### 阶段 5：发布替换

目标：

- 新版本安装包可发布。
- 老版本仍可回退。
- 首次启动自动备份 DB。

验收：

- GitHub Actions 能构建安装包。
- Release 资产完整。
- 安装后读取旧数据成功。

## 8. 测试策略

### 8.1 Rust 单元测试

覆盖：

- DB 读取旧 schema。
- 归档/恢复。
- 图片添加和删除。
- 设置读写。
- 迁移备份。

### 8.2 前端测试

覆盖：

- MessageCard 展开/收起。
- 多图缩略图布局。
- 编辑器保存失败状态。
- 设置页更新失败链接。

### 8.3 端到端测试

使用 Playwright 或 Tauri 测试方案，覆盖：

- 启动读取旧 DB。
- 新建图文消息。
- 编辑四图消息。
- 归档/恢复。
- 切换已归档大列表。
- 打开图片预览。

### 8.4 回归数据集

必须保留一个测试 DB，包含：

- 纯文字消息。
- 单图消息。
- 2-4 图消息。
- 18 图消息。
- 已归档消息。
- 空文本纯图片消息。
- 长文本消息。

## 9. 风险与对策

### 9.1 Windows 剪贴板复杂

对策：

- 剪贴板能力集中放 Rust 模块。
- 对复制文本、复制图片、导入外部窗口分别写 smoke。

### 9.2 图片路径兼容

对策：

- 第一阶段不移动图片。
- 所有图片通过 DB filename + images dir 解析。
- 找不到文件时显示缺失占位，同时写日志。

### 9.3 新旧版本同时运行

对策：

- MVP 阶段先只读。
- 写入阶段增加单实例锁。
- 写入前备份 DB。

### 9.4 Tauri/Rust 学习成本

对策：

- Rust 只写系统和数据层。
- UI 和业务状态尽量放 TypeScript。
- Commands 保持小函数，不做大而全服务。

## 10. AI 编辑友好约束

为了让后续和 AI 协作更稳定，必须遵守：

- 一个文件只承担一个清晰职责。
- UI 组件不直接访问 DB。
- Rust command 不直接拼 UI 状态。
- 类型集中定义。
- 所有异步操作有明确 loading/error/success 状态。
- 不用隐式全局变量保存业务状态。
- 复杂流程写状态机或流程注释。
- 每个阶段都要有可运行版本。

## 11. 首个 MVP 范围

建议新窗口第一轮只做 MVP-0：

```text
创建 clipstash-next
启动 Tauri + React + TypeScript
定位旧数据目录
读取旧 clipstash.db
显示消息总数、普通消息数、已归档消息数
```

不做：

- 不写 DB。
- 不做编辑器。
- 不做托盘。
- 不做快捷键。
- 不做导入。

这样可以先验证技术栈和数据兼容，不会伤害现有用户数据。

## 12. 推荐最终结论

采用：

```text
Tauri 2 + React + TypeScript + Rust + SQLite
```

迁移方式：

```text
旧 Python 版继续可用
新 Tauri 版从只读旧 DB 开始
逐阶段迁移功能
每阶段都可验证
最终用新版本替换旧版本
```

最重要的原则：

```text
先兼容，再替换。
先只读，再写入。
先 MVP，再完整。
先稳定，再漂亮。
```
