# 轻搜改名与发布体积优化

用户确认正式名称使用“轻搜”，包名和程序名统一为 qingso-launcher。窗口标题、搜索框页脚、设置页品牌和托盘名称已更新。

配置默认目录改为 QingsoLauncher。首次启动自动复制旧 FlowLauncherPreview 中的 hotkey.txt 和 search-tags.json，不删除旧文件且不覆盖新目录已有文件。新 QINGSO_HOTKEY / QINGSO_CONFIG_DIR 环境变量优先，旧 FLOW_* 变量继续兼容。单实例 IPC 标识保留旧值，防止旧版与新版同时运行并争抢快捷键。

发布配置新增 opt-level = "s" 和 strip = "symbols"，保留既有 LTO、codegen-units = 1、渲染器、托盘及无障碍功能。未改变 panic 策略。

Windows x64 同一份功能代码的发布基准为 14,063,616 字节（13.41 MiB），最终轻搜发布程序为 11,582,976 字节（11.05 MiB），减少约 17.64%。ZIP 分发包为 5,662,931 字节（5.40 MiB），包含 qingso-launcher.exe、使用说明.txt 和项目 LICENSE。程序图标资源通过 include_bytes! 内嵌。

产物位于 dist/windows-x64 和 dist/qingso-launcher-windows-x64-v0.1.0.zip，dist 不进入 Git。配置迁移与搜索单元测试、托盘 / 设置 / 快捷键检查、搜索 / 弹窗检查和严格 Clippy 检查通过。分发目录内的发布程序已成功启动。macOS 构建仍需在 Mac 上验证。
