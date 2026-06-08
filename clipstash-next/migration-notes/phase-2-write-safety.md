# 阶段 2 写入安全策略

## 最小范围

- 第一批写入只做新增消息。
- 新增文字消息写入 `messages(text_content, archived)`。
- 新增图片或图文消息继续写入旧 `images/`，再写入 `message_images(message_id, image_filename)`。
- 不做编辑、删除、归档、恢复、导入和迁移。

## 兼容约束

- 不删除旧表。
- 不重命名旧字段。
- 不改变 `messages` 和 `message_images` 的语义。
- 图片顺序继续依赖 `message_images.id ASC`。
- 写入后的数据必须能被旧 Python 版读取。

## 写前备份

- 首次真实写入前必须调用 Rust 备份函数。
- 备份文件放在旧 DB 同目录，命名为 `clipstash.db.bak-YYYYMMDD-HHMMSS`。
- 备份失败时中止写入。
- 备份函数必须可用临时数据库测试，不直接依赖真实用户数据。

## 回滚方式

- 关闭新应用。
- 将当前 `clipstash.db` 改名保留。
- 将目标备份文件改回 `clipstash.db`。
- `images/` 中新增图片文件需要按写入日志或 DB 记录手动清理；阶段 2 实现写入时必须补充这部分日志。

## 下一步

- 暴露一个内部写入前检查流程。
- 实现新增纯文字消息，并在临时数据库中验证旧 schema 可读。
- 对真实旧库写入前，先人工确认备份路径和回滚步骤。
