# 登录自启开关

用户要求新增开机是否自启。在常规设置添加默认关闭的开关，读取当前用户系统登录启动项作为状态来源。启用时记录当前程序路径和 --autostart 参数，关闭仅删除轻搜自己的启动项。操作失败显示错误并恢复状态。

Windows 通过 winreg 写入 HKCU\Software\Microsoft\Windows\CurrentVersion\Run 的 QingsoLauncher 值，路径用引号包围，支持空格和中文。macOS 写入当前用户 Library/LaunchAgents/net.qingso.launcher.plist，ProgramArguments 指定程序路径和 --autostart，RunAtLoad 为 true，GUI 会话限制为 Aqua。macOS 代码尚未实机验证。

主程序检查 --autostart，登录启动时创建托盘并保留后台事件循环，不显示搜索框。单实例入口接受是否唤回的参数，重复登录启动直接正常退出，不发送显示消息；手动重复启动维持唤回行为。

五项单元测试通过，其中自启新增测试使用隔离的 HKCU 注册表测试键验证读写及删除，并验证引号路径、超长路径和 plist XML 转义。未修改当前用户真实 Run 项。后台启动及重复后台启动经实际可见窗口检查通过；Winit 的 14×14 内部事件窗口不计为应用窗口。设置 / 托盘 / 快捷键检查和严格 Clippy 检查通过。
