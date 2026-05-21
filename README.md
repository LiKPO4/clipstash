# ClipStash

ClipStash（需求暂存站）是一个 Windows 剪贴板暂存工具，用来快速保存文字和图片素材，并在需要时一键复制或粘贴。

## 功能

- 保存文字、图片或图文混合消息
- 图片悬浮预览
- 消息归档与排序
- 托盘驻留和 `Ctrl+Shift+V` 呼出
- 从 GitHub Releases 检查新版本

## 本地运行

```powershell
python -m pip install -r requirements.txt
python main.py
```

## 打包（安装包）

为了**解决 `--onefile` 启动慢**（每次运行都要解压到临时目录）的问题，现在改用 `--onedir` + **Inno Setup** 打包。

用户最终得到的是一个 `ClipStash-Setup-vX.X.X.exe`，安装后在桌面生成快捷方式。

### 环境要求

- Python + PyInstaller
- [Inno Setup 6](https://jrsoftware.org/isdl.php)（安装后 `ISCC.exe` 自动加入 PATH）

### 一键打包

```powershell
cd setup
.\build.ps1 -Version "1.3.9"
```

生成文件位于 `dist/ClipStash-Setup-v1.3.9.exe`。

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
