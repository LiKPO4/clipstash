# 阶段 4 新库与切换策略

## 目标

- 在阶段 2/3 已验证旧库读写兼容的基础上，规划从“直接操作旧库”过渡到“可维护的新数据层”。
- 新旧版本继续可并行测试，任何迁移动作都必须可审计、可回滚。
- 不破坏旧 `messages`、`message_images` 和旧 `images/` 目录。

## 当前事实

- Next 已能读取旧 `clipstash.db` 和旧 `images/`。
- Next 已通过真实 UI 验证新增纯图片、新增图文、编辑文字、替换图片、删除、归档/恢复、复制、导入队列和受控粘贴。
- 当前旧库基线为 `normal=11 archived=103 total=114 max_id=114 joined_images=107 orphan_images=0`。
- 阶段 2/3 写库验收均已清理测试消息，旧库已回到基线。

## 策略结论

短期不立刻迁移到独立新库。下一阶段先把数据访问层整理为“双模式”：

- `legacy mode`：继续直接读取和写入旧 schema，作为默认运行模式。
- `shadow mode`：在不改变用户行为的前提下，生成新 schema 的影子索引或影子库，用于比对和迁移演练。
- `cutover mode`：只有在影子校验长期稳定后，才允许把新 schema 作为主写入目标。

## 新 schema 边界

新 schema 只能新增，不得替换旧 schema 的核心表语义。

建议新增：

```sql
CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS message_index (
    message_id INTEGER PRIMARY KEY,
    text_length INTEGER NOT NULL DEFAULT 0,
    image_count INTEGER NOT NULL DEFAULT 0,
    has_missing_images INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS migration_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL,
    detail_json TEXT
);
```

暂不新增新的主消息表。原因：

- 旧 Python 版仍依赖 `messages` 和 `message_images`。
- 当前真实 UI 写库路径已经证明旧 schema 可继续承载 MVP。
- 先做影子索引能提升可观测性，而不会制造双写冲突。

## 阶段拆分

### 4A：数据访问层整理

- 把 Rust 侧旧库读写入口按职责拆清楚：定位、查询、写入、备份、图片文件。
- 保持现有 Tauri command 入参和返回结构不变。
- 为每个写入入口补充统一审计结构：操作名、消息 id、DB 备份路径、图片备份路径。

验收：

- `cargo test` 通过。
- `npm test` 通过。
- `npm run verify:legacy-readonly` 通过。
- 真实旧库基线不变。

### 4B：影子索引

- 新增 `message_index` 和 `migration_audit`，仅记录可由旧库重新计算出的派生信息。
- 首次构建影子索引前备份 DB。
- 构建后执行一致性检查：消息数、图片关联数、缺失图片数必须可解释。

验收：

- 旧 Python 版仍能读取旧库。
- Next 仍能读取旧库。
- `messages` 和 `message_images` 行数不因影子索引变化。
- 影子索引可删除后重建。

### 4C：设置与系统能力迁移

- 把设置读取/写入、快捷键、托盘、开机启动、更新检查纳入 Next。
- 设置数据优先继续兼容旧 `settings.json`，新增字段必须有默认值。
- 更新检查失败时必须提供 Release 页面入口。

验收：

- 设置保存后重启仍生效。
- 快捷键保存和恢复可验证。
- 托盘菜单可显示、隐藏、退出。
- 更新失败提示清晰且可跳转 Release 页面。

### 4D：发布替换预演

- 新增构建与安装包流程。
- 安装后默认仍使用旧数据目录。
- 首次启动先备份 DB，再执行只读审计。

验收：

- 新安装包启动后能显示旧库计数。
- 旧 Python 版仍可启动并读取同一旧库。
- 回滚到旧版本时，不需要迁移回退操作。

## 回滚策略

- 代码回滚：使用对应 git commit 的 `git revert`。
- DB 回滚：关闭新旧应用，把目标 `clipstash.db.bak-*` 复制回 `clipstash.db`。
- 影子表回滚：允许删除 `app_meta`、`message_index`、`migration_audit`，不得删除旧核心表。
- 图片回滚：优先使用 `images.bak-*`，不得批量清理未知图片。

## 下一步最小行动

先做 4A：整理 Rust 数据访问层的审计返回结构，但不改变 UI 行为、不改变旧 schema、不迁移数据。
