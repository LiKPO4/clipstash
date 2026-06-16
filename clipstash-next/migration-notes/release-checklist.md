# ClipStash Next 发布与回滚清单

## 构建命令

在 `clipstash-next/` 下执行：

```powershell
npm test -- --run
npm run build
cd src-tauri
cargo fmt -- --check
cargo test
cd ..
npm run tauri build
npm run verify:legacy-readonly
```

迁移去重与旧库不变性的隔离测试：

```powershell
cd src-tauri
cargo test migrates_legacy_data_once_and_skips_duplicates_without_touching_legacy_files
```

## 发布产物

版本号来自 `clipstash-next/src-tauri/tauri.conf.json` 和 `clipstash-next/package.json`，当前为 `2.1.9`。

- 主程序：`clipstash-next/src-tauri/target/release/clipstash-next.exe`
- MSI：`clipstash-next/src-tauri/target/release/bundle/msi/ClipStash Next_2.1.9_x64_en-US.msi`
- NSIS：`clipstash-next/src-tauri/target/release/bundle/nsis/ClipStash Next_2.1.9_x64-setup.exe`
- Android release APK：`clipstash-next/src-tauri/gen/android/app/build/outputs/apk/universal/release/ClipStash.Next_2.1.9_android-universal-release-signed.apk`

GitHub Release 上传 Windows 用户优先使用 NSIS 安装包，同时保留 MSI 作为备用安装入口；Android 版上传 release 构建签名通用 APK，用于侧载验收。

## 数据目录策略

- 旧 Python 版数据目录：`%APPDATA%\ClipStash`
- Next 版数据目录：`%APPDATA%\ClipStash Next`
- Next 首次启动会只读扫描旧库，并复制旧图片到 Next 数据目录。
- Next 日常读写、复制、导入、归档、删除只操作 `%APPDATA%\ClipStash Next`。
- Next 不移动、不删除、不覆盖旧 `clipstash.db` 或旧 `images/`。
- 设置页底部“迁移旧数据”可重复触发；迁移逻辑会跳过重复消息和重复图片，避免把同一条旧数据重复写入新库。
- `migrates_legacy_data_once_and_skips_duplicates_without_touching_legacy_files` 覆盖：首次迁移、重复迁移跳过、缺失图片引用保留、旧 DB 字节不变、旧图片字节不变。

## 回滚旧 Python 版

回滚时直接关闭 ClipStash Next，再启动旧 Python 版或旧安装包即可。旧版继续读取 `%APPDATA%\ClipStash\clipstash.db` 和 `%APPDATA%\ClipStash\images`，Next 的新数据目录不会影响旧版。

如果用户已在 Next 中新增数据，回滚旧版不会自动带回这些新消息；需要保留 Next 数据目录，等待后续导出或同步工具处理。

## 发布前人工验收

下面项目需要真实 Windows 桌面环境验收。每项失败时记录：操作步骤、屏幕截图、设置页错误文案、`%APPDATA%\ClipStash Next` 当前文件状态。

### 1. 安装与首次迁移

1. 关闭正在运行的 ClipStash Next。
2. 运行 NSIS 安装包：`clipstash-next/src-tauri/target/release/bundle/nsis/ClipStash Next_2.1.9_x64-setup.exe`。
3. 启动 ClipStash Next。
4. 打开设置页，确认“本地存储”指向 `%APPDATA%\ClipStash Next`。
5. 确认 `%APPDATA%\ClipStash Next\clipstash.db` 和 `%APPDATA%\ClipStash Next\images` 存在。
6. 确认旧 `%APPDATA%\ClipStash\clipstash.db` 与 `%APPDATA%\ClipStash\images` 仍存在。

预期：Next 能启动；新数据目录存在；旧数据目录未被移动、删除或覆盖。

### 2. 手动重复迁移

1. 在设置页点击“迁移旧数据”。
2. 记录提示中的“新增/跳过/复制图片”数量。
3. 再次点击“迁移旧数据”。

预期：第二次迁移应主要显示跳过重复，不应把同一批旧消息重复写入新库。

### 3. 旧 Python 版回滚

1. 退出 ClipStash Next。
2. 启动旧 Python 版 ClipStash。
3. 检查旧版普通/归档列表可打开，旧图片可预览。

预期：旧 Python 版仍读取 `%APPDATA%\ClipStash`，不受 Next 新数据目录影响。

### 4. 主界面视觉与列表

1. 确认主窗口默认宽度约 `370`。
2. 切换普通/已归档列表。
3. 滚动接近列表底部，确认会自动加载更多。
4. 找到多图消息，展开/收起图片。
5. 找到缺图消息或使用回归数据集副本验证缺图占位。
6. 鼠标悬停图片，确认预览贴近原图、不遮挡原图，长边最大约 `1000`。

预期：列表不卡死、不出现多余页面滚动条；图片预览位置和尺寸符合旧版体验。

### 5. 设置持久化

1. 在设置页修改：排序、导入后归档、关闭窗口时隐藏到托盘、悬浮预览延迟、滚动速度、字体大小。
2. 关闭并重新启动 Next。
3. 再次打开设置页检查这些值。

预期：重启后设置仍生效；设置页不显示虚假成功。

### 6. 托盘与窗口行为

1. 设置“关闭窗口时隐藏到托盘”为开启。
2. 关闭主窗口。
3. 通过托盘菜单“显示/隐藏主窗口”恢复窗口。
4. 通过托盘菜单“打开数据目录”打开 `%APPDATA%\ClipStash Next`。
5. 通过托盘菜单“退出”退出应用。
6. 再设置“关闭窗口时隐藏到托盘”为关闭，关闭窗口。

预期：开启时关闭窗口只隐藏不退出；托盘退出后进程结束；关闭该设置后关闭窗口会退出。

### 7. 全局快捷键

1. 切换到其他应用窗口。
2. 按 `<ctrl>+<shift>+v`。
3. 再按一次 `<ctrl>+<shift>+v`。
4. 复制一段文本或一张图片到系统剪贴板。
5. 按 `<ctrl>+<alt>+v`。

预期：呼出快捷键能显示/隐藏 Next；快速保存快捷键能把当前剪贴板内容保存到 Next 新库，不写旧库。

### 8. 主窗口剪贴板入口

1. 在其他应用复制文字。
2. 聚焦 ClipStash Next 主窗口空白处。
3. 按 `Ctrl+V`。
4. 确认新建/编辑窗口带入文字。
5. 复制图片后重复上述步骤。

预期：文字或图片进入统一新建窗口，图片有缩略图和悬停预览。

### 9. 开机自启动

1. 设置页开启“开机自启动”。
2. 关闭并重启 Next，确认开关仍开启。
3. 注销或重启 Windows。
4. 登录后检查 Next 是否自动启动。
5. 关闭“开机自启动”，再次注销或重启验证不会自启动。

预期：系统启动项状态与 UI 一致。

### 10. 更新检查失败

自动化测试已覆盖网络异常、HTTP 失败和响应缺字段。人工可选验收：

1. 断网或阻断 GitHub API。
2. 设置页点击“检查更新”。

预期：显示明确失败原因，不出现“待实现”或模糊提示。
