# Flow 风格跨平台启动器

基于 Rust + Slint 的桌面启动器界面原型，参考 Flow Launcher 的搜索框与结果列表布局。

当前仅实现样式：深色无边框窗口、搜索输入框、分类标签、六条静态示例结果、选中与悬停状态、SVG 图标和底部快捷键提示。输入框可以输入文字，Esc 或右上角 esc 按钮可以关闭窗口。分类切换、结果导航、搜索、启动应用和全局快捷键尚未接入；界面里的快捷键是视觉示例。

结果区最多同时显示三条，每条 54 逻辑像素。内容超过三条时自动显示垂直滚动条，可通过鼠标滚轮、触控板或拖动滚动条查看其余结果；三条及以下不显示滚动条。窗口逻辑尺寸为 760 × 386。

## 运行

安装 Rust 工具链后，在项目目录执行：

```sh
cargo run
```

Windows 和 macOS 使用同一套 Slint 界面。Windows 使用 Segoe UI 和 Alt / Ctrl 标识；macOS 使用 Helvetica Neue 和 ⌘ 标识，终端及系统设置名称也会随平台变化。当前在 Windows 完成构建及实际渲染检查，macOS 仍需在 Mac 上验证构建、字体和窗口表现。当前深色背景为实色，不依赖系统毛玻璃 API。

## 文件

- `ui/app-window.slint`：窗口、搜索区、结果行及快捷键组件。
- `ui/icons/`：内嵌 SVG 图标，避免系统 emoji 的样式和字形差异。
- `src/main.rs`：平台显示适配及窗口关闭。
- `src/window_placement.rs`：等待原生窗口尺寸就绪后，在窗口所在屏幕居中；使用物理像素和屏幕原点处理缩放及副屏坐标。
- `examples/render-preview.rs`：从 Slint 渲染器导出 BMP，用于检查真实布局。
- `preview.png`：Windows 环境下的界面预览。

## 检查和预览

```sh
cargo check
cargo build
cargo run --example render-preview -- preview.bmp
```

预览导出会短暂显示窗口，再自动关闭。它使用系统缩放比例，输出图像尺寸可能大于 760 × 548 的逻辑窗口尺寸。
