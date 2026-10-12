# Flow 风格跨平台启动器

基于 Rust + Slint 的桌面启动器界面原型，参考 Flow Launcher 的搜索框与结果列表布局。

当前实现浅色无边框窗口、搜索输入框、分类标签、六条静态示例结果、选中与悬停状态、SVG 图标及基础启动器交互。上下方向键可选择结果，选中项自动滚动到可见区域；点击结果也可选择。网站标签搜索已接入，匹配标签时按 Enter 可打开搜索结果。应用 / 文件搜索、分类切换及应用启动仍未接入，Alt / ⌘ 数字及 Ctrl / ⌘ K 仍是视觉提示。

结果区最多同时显示三条，每条 54 逻辑像素。内容超过三条时自动显示垂直滚动条，可通过鼠标滚轮、触控板或拖动滚动条查看其余结果；三条及以下不显示滚动条。窗口逻辑尺寸为 760 × 386。

## 运行

安装 Rust 工具链后，在项目目录执行：

```sh
cargo run
```

Windows 和 macOS 使用同一套 Slint 界面。Windows 使用 Segoe UI 和 Alt / Ctrl 标识；macOS 使用 Helvetica Neue 和 ⌘ 标识，终端及系统设置名称也会随平台变化。当前在 Windows 完成构建及实际渲染检查，macOS 仍需在 Mac 上验证构建、字体和窗口表现。当前浅色背景为实色，不依赖系统毛玻璃 API。

## 快捷键和窗口行为

- 系统托盘提供“打开搜索框”“打开设置”“退出”；点击托盘图标可直接打开搜索框。
- 设置是独立窗口，显示软件名称、版本、运行平台、当前快捷键和配置文件位置。关闭设置窗口不会退出后台程序。
- 设置页使用浅色侧边栏导航、信息卡片、固定字段标题和统一圆角操作按钮；内容宽度会随窗口宽度调整。
- 在设置页输入快捷键组合并点击“保存快捷键”，立即更新注册并保存。无效格式、被占用或保存失败会显示错误，原快捷键继续有效。
- 程序只运行一个实例。重复执行 `cargo run` 或双击程序会唤回已有窗口，新进程正常退出，不会再次争用全局快捷键。
- 默认全局快捷键为 `Alt+Space`；如果已被其他软件注册，自动尝试 `Ctrl+Alt+Space`。窗口底部显示实际快捷键。
- 按快捷键显示窗口，再按一次隐藏；显示时重新居中并聚焦搜索框。
- Esc、右上角 esc 按钮或点击其他窗口会隐藏启动器。隐藏后保持后台运行，搜索文字和选择状态保留。
- Windows 窗口跳过任务栏，双击 debug / release 程序均不会额外创建控制台窗口；macOS 使用 Accessory 激活策略隐藏 Dock 图标。
- 在窗口中按 `Ctrl+Shift+Q` 完全退出并释放快捷键。

全局快捷键由 [global-hotkey](https://docs.rs/global-hotkey/0.8.0/global_hotkey/) 注册，管理器和原生事件循环位于主线程。

也可在 PowerShell 中指定快捷键：

```powershell
$env:FLOW_HOTKEY = 'Ctrl+Alt+Space'
cargo run
```

主动指定的快捷键无法注册时返回启动错误，不会偷偷更换为其他快捷键。默认两个快捷键都被占用时，也需要通过 `FLOW_HOTKEY` 指定其他组合。

启动时按 `FLOW_HOTKEY` 环境变量、设置页保存值、默认组合的顺序选择快捷键。Windows 默认保存到 `%APPDATA%/FlowLauncherPreview/hotkey.txt`，macOS 保存到 `~/Library/Application Support/FlowLauncherPreview/hotkey.txt`。也可用 `FLOW_CONFIG_DIR` 指定独立配置目录。

启动失败时会弹出具体错误信息。Windows 的后台子系统不创建控制台，因此错误窗口可避免只看到 Cargo 的 `exit code: 1`。

## 搜索标签

在“设置 → 搜索标签”中添加、编辑或删除网站搜索标签。内置 `b` 对应百度、`n` 对应 npm：

```text
b Rust 中文教程
n @slint-ui/slint-ui
```

搜索框使用“标签 + 空格 + 搜索内容”。匹配后显示网站名称，按 Enter 或点击对应结果，用默认浏览器打开网址，成功后隐藏搜索框。空内容、未知标签和浏览器调用失败会显示原因。

每个标签包含标签、网站名称和网址模板，模板中必须包含 `{query}`。例如百度为 `https://www.baidu.com/s?wd={query}`，npm 为 `https://www.npmjs.com/search?q={query}`。标签不区分大小写，不允许重复；搜索内容会做网址编码，支持中文、空格和特殊字符。网址支持 http / https。

配置保存到与快捷键文件同目录的 `search-tags.json`，修改立即生效并在下次启动恢复。删除全部标签后保持空列表，不会重新自动添加默认标签。

## 文件

- `ui/app-window.slint`：窗口、搜索区、结果行及快捷键组件。
- `ui/icons/`：内嵌 SVG 图标，避免系统 emoji 的样式和字形差异。
- `src/main.rs`：平台显示适配及窗口关闭。
- `src/launcher.rs`：平台窗口配置、失焦隐藏、键盘选择及唤回聚焦。
- `src/hotkeys.rs`：全局快捷键注册、冲突回退和按下 / 松开处理。
- `src/instance.rs`：OS 单实例锁和本地 IPC，重复启动唤回已有窗口。
- `src/settings.rs`：设置窗、托盘菜单和快捷键保存回调。
- `src/config.rs`：用户配置读取和原子替换保存。
- `ui/settings-window.slint`：设置页与原生系统托盘菜单。
- `ui/theme.slint`：统一浅色配色，供搜索框、设置页和启动提示窗口使用。
- `src/search_tags.rs`：标签校验、持久化、输入匹配和网址生成。
- `src/search.rs`：设置标签回调、搜索提示和默认浏览器打开。
- `src/window_placement.rs`：等待原生窗口尺寸就绪后，在窗口所在屏幕居中；使用物理像素和屏幕原点处理缩放及副屏坐标。
- `examples/render-preview.rs`：从 Slint 渲染器导出 BMP，用于检查真实布局。
- `preview.png`：Windows 环境下的界面预览。

## 检查和预览

```sh
cargo check
cargo build
cargo run --example render-preview -- preview.bmp
cargo run --example interaction-check
cargo run --example settings-check
cargo run --example render-settings
cargo run --example render-settings -- tags
cargo run --example search-check
```

预览导出会短暂显示窗口，再自动关闭。它使用系统缩放比例，输出图像尺寸可能大于 760 × 386 的逻辑窗口尺寸。

交互检查会注册快捷键并短暂显示测试窗口，验证上下键边界、自动滚动、Esc 隐藏、后台循环、重复切换以及原生失焦隐藏。Windows 下还向测试进程自己的快捷键管理窗口注入消息，验证原生快捷键消息到 Slint 显示切换的链路。该检查应在实际启动器退出后运行。

设置检查使用独立临时配置目录，验证原生托盘创建、打开窗口、快捷键格式与占用错误、保存失败保护、热更新、重新读取配置和托盘退出。`settings-preview.png` 为设置界面的实际渲染预览。

搜索检查使用独立配置目录和捕获网址的浏览器替身，验证真实 Enter 回调、默认标签、添加 / 编辑 / 删除即时生效及持久化、网址编码和错误反馈，不会实际打开浏览器。`tags-preview.png` 为标签页预览。
