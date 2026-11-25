# 需求暂存站 v2.3.5

## 改进

- 桌面右键菜单与 Android 长按菜单中的「钉住置顶」「钉住置底」精简为「置顶」「置底」。
- 已设置状态仍显示「取消置顶」「取消置底」，置顶、置底及切换逻辑不变。

## 安装包

- Windows：提供 EXE 安装包和 MSI 备用安装包。
- Android：提供正式签名通用 APK，可覆盖安装已有正式版本。

## 验证

- 前端：191 项通过、9 项跳过，`npm run build` 通过。
- Rust：113 项通过、22 项忽略，`cargo fmt -- --check` 通过。
- 旧库只读一致性校验通过：normal=5、archived=112、total=117、joined_images=130、orphan_images=0。
- Windows 与 Android 安装包由 GitHub Actions 从 `v2.3.5` 标签云端构建并自动发布，不上传本地包体。
- 本次未进行 Windows / Android 真机验收。
