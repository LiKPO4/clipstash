# 回归测试数据集

## 路径

生成后的旧版兼容数据集位于：

`clipstash-next/test-data/regression/legacy/`

目录结构：

- `clipstash.db`
- `images/`

## 覆盖内容

当前数据集覆盖：

- 纯文字消息
- 文字 + 单图
- 文字 + 4 图
- 文字 + 18 图
- 已归档消息
- 纯图片消息
- 长文本消息
- 缺失图片引用

生成后只读审计结果应为：

- `normal=7`
- `archived=1`
- `total=8`
- `joined_images=26`
- `orphan_images=0`

## 生成与校验

在 `clipstash-next/src-tauri/` 下执行：

```powershell
cargo test generates_regression_fixture -- --ignored --nocapture
cargo test verifies_regression_fixture_readonly_consistency -- --ignored --nocapture
```

生成命令会重建 `clipstash-next/test-data/regression/legacy/`，只写仓库内测试目录，不读写真实 `%APPDATA%\ClipStash`。

## 使用方式

该数据集用于本地 UI 或只读一致性回归检查。需要人工 UI 验收时，可临时把应用数据目录指向该夹具的副本，或在开发环境中把该目录复制到隔离的测试用户数据目录。不要用它覆盖真实旧库。
