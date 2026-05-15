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

## 打包

```powershell
python -m PyInstaller --noconfirm --onefile --windowed --name ClipStash main.py
```

生成文件位于 `dist/ClipStash.exe`。
