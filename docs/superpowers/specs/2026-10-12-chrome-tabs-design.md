# Chrome 标签搜索

用户确认使用配套 Chrome 扩展，支持 Windows 和 macOS。

- `ll` 后接空白进入浏览器标签模式；关键词按标题及网址匹配，忽略大小写，多个词均须匹配。无关键词显示全部标签。
- 列表沿用三条可见、更多结果滚动、上下键选择和点击/回车打开，操作为激活已有标签并聚焦所在 Chrome 窗口。
- Chrome Manifest V3 扩展只请求 tabs、nativeMessaging、alarms。由官方 Native Messaging 连接轻搜，使用用户级本地 IPC，不开放远程调试端口。
- 原生宿主复用轻搜可执行文件，在 UI 初始化前识别扩展 origin，纯 stdio 模式不会出现托盘、窗口或注册快捷键。标签仅保存在内存，退出连接立即移除；每个 Chrome 配置文件单独连接，标签 ID 与连接 ID 共同标识目标。
- 设置新增 Chrome 标签页面：导出内置扩展、注册当前程序为本机宿主、打开安装指引、显示连接/标签数量。固定扩展公钥确保 ID 稳定。Windows 注册 HKCU NativeMessagingHosts，macOS 写用户 Chrome NativeMessagingHosts 清单；移动程序后重新配置。
- 输入更新只筛选内存数据，不阻塞 UI。扩展监听标签变化并同步；激活请求带请求 ID，成功后隐藏，失败显示错误，连接关闭清理结果。无扩展、无匹配、目标已关闭均有明确提示。ll 为内置保留标签。
- 验证：Rust 协议、过滤、断连/多配置文件路由；JS 模拟 Chrome API 验证同步及激活；Windows 原生宿主 stdio 与 IPC 集成；既有搜索/推荐/窗口交互回归；macOS 注册及 stdio 实机检查需在 Mac 完成。
