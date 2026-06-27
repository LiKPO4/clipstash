# Android 小组件基本设计

## 目标

在 Android 桌面创建一个类似参考图中间卡片的 ClipStash 小组件，让用户不打开应用也能看到需要处理的暂存内容。

本阶段先做基本设计，后续实现时优先做一个稳定、低风险的原生 Android AppWidget，不把 React WebView 直接放进小组件。

## 设计原则

- 小组件只展示最需要扫一眼的信息，不承载完整管理流程。
- 视觉接近系统桌面小组件：白色圆角卡片、轻阴影、标题强调色、列表大字号、触控目标明确。
- 数据读取必须快，不能依赖应用已经打开。
- 点击小组件内容进入 App 对应页面处理，避免在小组件里做复杂编辑。
- Android 原生小组件用 `RemoteViews` 实现，不能直接复用 Tauri/React 页面。

## MVP 形态

### 小组件名称

`需求暂存站 - 待办`

### 支持尺寸

- 首发只支持 `4 x 2`，对应参考图中间的宽卡片。
- 后续再加 `4 x 3` 或 `4 x 4`，用于显示更多条。

### 视觉结构

- 卡片背景：白色或跟随系统浅色，圆角约 `24dp`。
- 顶部：
  - 左侧标题：`待办`
  - 标题右侧数量：普通消息数量或待办数量，例如 `16`
  - 右侧按钮：新建图标，点击打开 App 的新建消息界面。
- 内容区：
  - 最多展示 3 条文本消息。
  - 每条左侧一个空心圆状态点。
  - 右侧文本单行展示，超出省略。
  - 图片消息显示为 `[图片]`，图文消息优先显示文字摘要。
- 底部：
  - 小字 App 名称：`需求暂存站`

### 交互

- 点击某条消息：打开 App，并定位或高亮该消息。
- 点击右上角新建：打开 App 的新建消息弹窗或新建页面。
- 点击卡片空白处：打开 App 首页。
- 桌面刷新由系统触发，App 内新增/删除/归档后主动通知小组件刷新。

## 数据范围

MVP 默认展示普通消息，也就是当前 App 首页“普通”列表中的最新 3 条。

原因：

- 现有数据模型已经区分普通/归档，普通消息最接近“待办”。
- 不需要新增数据库表，风险低。
- 和用户参考图里的“待办”语义一致。

后续可扩展一个小组件设置：

- 展示最新普通消息
- 展示置顶/固定消息
- 展示最近创建
- 展示指定关键词或标签

## 原生 Android 落点

建议新增文件：

- `clipstash-next/src-tauri/gen/android/app/src/main/java/com/clipstash/next/ClipStashWidgetProvider.kt`
- `clipstash-next/src-tauri/gen/android/app/src/main/java/com/clipstash/next/ClipStashWidgetData.kt`
- `clipstash-next/src-tauri/gen/android/app/src/main/res/layout/widget_todo.xml`
- `clipstash-next/src-tauri/gen/android/app/src/main/res/xml/clipstash_widget_info.xml`
- `clipstash-next/src-tauri/gen/android/app/src/main/res/drawable/widget_card_background.xml`
- `clipstash-next/src-tauri/gen/android/app/src/main/res/drawable/widget_status_circle.xml`

同时在 `AndroidManifest.xml` 注册 `receiver`：

- `android.appwidget.action.APPWIDGET_UPDATE`
- `@xml/clipstash_widget_info`

## 数据读取方案

小组件不能依赖 Rust/Tauri command，因为它在桌面上由系统进程周期性更新，不一定有 WebView 或 Tauri runtime。

MVP 推荐 Kotlin 直接只读 SQLite：

- 数据库路径沿用 App 当前 Android 数据目录下的新库。
- 只读查询普通消息，按创建时间倒序取 3 条。
- 查询字段：`id`、`text_content`、`created_at`、图片数量。
- 读取失败时展示空状态：`暂无待办`

如果 Android 新库路径或表结构变动，必须同步更新 `ClipStashWidgetData.kt`，并补一条小组件只读查询测试或手动验收记录。

## App 内刷新触发

需要在 Android 端增加一个桥接函数或原生 helper：

- 新建消息成功后刷新小组件。
- 删除消息成功后刷新小组件。
- 归档/恢复成功后刷新小组件。
- 导入数据成功后刷新小组件。

实现上可以在 `MainActivity.kt` 暴露：

- `ClipStashAndroid.refreshWidgets()`

前端在 Android 平台、相关写操作成功后调用。后续也可以改为 Rust command 调用 Android 原生层，但 MVP 用现有 JavaScript bridge 更轻。

## 空状态和异常状态

- 没有普通消息：标题显示 `待办 0`，内容显示 `暂无待办`。
- 数据库不存在：显示 `打开应用完成初始化`。
- 数据读取失败：显示 `小组件暂不可用`，点击可打开 App。

## 分阶段实现

### 阶段 1：静态可添加

- 注册 AppWidget。
- 桌面能添加 `需求暂存站 - 待办`。
- 使用假数据渲染参考图风格。
- 验证 APK 可安装，桌面小组件列表可见。

状态：已实现原生静态小组件入口，APK Manifest 已包含 `ClipStashWidgetProvider`、`android.appwidget.action.APPWIDGET_UPDATE` 和 `android.appwidget.provider` metadata；真实手机桌面添加和视觉验收仍待安装后确认。

### 阶段 2：只读真实数据

- Kotlin 只读 SQLite。
- 展示最新 3 条普通消息和普通消息总数。
- 点击条目打开 App。

状态：已实现 `ClipStashWidgetData.kt`，小组件会读取 Android 当前数据目录中的 `clipstash.db`，支持 `data-location.json` 迁移目录；读取普通消息总数和最新 3 条普通消息，纯图片消息显示 `[图片]`，空库显示 `暂无需求`，数据库不存在显示 `打开应用完成初始化`，读取失败显示 `小组件暂不可用`。

### 阶段 3：App 内主动刷新

- 新建、删除、归档、恢复、导入后刷新小组件。
- 首次打开 App 后刷新一次小组件。

状态：已实现最小主动刷新。`MainActivity` 通过 `ClipStashAndroid.refreshWidgets()` 暴露原生刷新入口，前端在 Android 新建、编辑、删除、归档/恢复、数据包导入、分享创建消息成功后调用刷新。首次打开 App 后自动刷新仍待补。

### 阶段 4：创建小组件入口

- Android 设置页新增“小组件”分组。
- 提供“添加到桌面”的说明和系统小组件选择入口。
- 如果系统/启动器不支持直接 pin widget，则显示引导文案。

## 验收清单

- Android release APK 可以正常构建。已验证。
- 手机桌面长按添加小组件时能看到 ClipStash 小组件。待真机验证。
- 添加后视觉接近参考图：白色圆角卡片、`待办 数量`、3 条内容、右上角新建图标。待真机验证。
- App 未打开时，小组件仍能展示上次/当前数据库内容。
- 新建普通消息后，小组件内容刷新。
- 归档消息后，小组件数量和列表刷新。
- 点击卡片能打开 App。
- 点击新建图标能进入新建流程。

## 风险

- 不同 Android 启动器对小组件圆角、背景、直接 pin 行为支持不同，视觉会有细微差异。
- Tauri 生成的 Android 工程后续重新生成时，原生 Kotlin 和 res 文件需要确认不会被覆盖。
- RemoteViews 支持的布局能力有限，不能使用复杂自定义字体、动态渐变或 WebView。
- Android 后台限制较严，刷新应以写操作后主动刷新为主，不依赖高频定时刷新。
