# ClipStash

ClipStash（需求暂存站）是一个剪贴板暂存工具，用来快速保存文字和图片素材，并在需要时一键复制或粘贴。

当前仓库包含两个版本：

- **ClipStash Next（推荐）**：`clipstash-next/`，基于 Tauri 2 + React + TypeScript + Rust + SQLite，当前版本 `v2.2.0`，支持 Windows 桌面与 Android。
- **旧版 Python/Tkinter**：根目录，当前版本 `v1.3.43`，继续可用，但不再新增大功能。

Next 版首次启动会只读扫描旧版 `%APPDATA%\ClipStash` 数据并复制到 `%APPDATA%\ClipStash Next`，不会破坏旧版数据。

## 功能

- 保存文字、图片或图文混合消息
- 图片悬浮预览
- 消息归档与排序
- 托盘驻留和 `Ctrl+Shift+V` 呼出
- 从 GitHub Releases 检查新版本

## ClipStash Next（Tauri）

在 `clipstash-next/` 目录下：

```powershell
cd clipstash-next
npm install
npm run tauri dev
```

构建安装包：

```powershell
cd clipstash-next
npm run tauri build
```

发布清单与回滚策略见 `clipstash-next/migration-notes/release-checklist.md`。

## 旧版 Python/Tkinter

### 本地运行

```powershell
python -m pip install -r requirements.txt
python main.py
```

### 打包（安装包）

为了**解决 `--onefile` 启动慢**（每次运行都要解压到临时目录）的问题，现在改用 `--onedir` + **Inno Setup** 打包。

用户最终得到的是一个 `ClipStash-Setup-vX.X.X.exe`，安装后在桌面生成快捷方式。

#### 环境要求

- Python + PyInstaller
- [Inno Setup 6](https://jrsoftware.org/isdl.php)（安装后 `ISCC.exe` 自动加入 PATH）

#### 一键打包

```powershell
cd setup
.\build.ps1 -Version "1.3.43"
```

生成文件位于 `dist/ClipStash-Setup-v1.3.43.exe`。

### 手动分步打包

```powershell
# 1. PyInstaller --onedir
python -m PyInstaller `
    --noconfirm --onedir --windowed `
    --name ClipStash `
    --icon assets/app_icon.ico `
    --add-data "assets;assets" `
    --exclude-module PyQt5 --exclude-module PyQt6 `
    --exclude-module PySide2 --exclude-module PySide6 `
    main.py

# 2. Inno Setup
ISCC.exe setup\ClipStash.iss
```
