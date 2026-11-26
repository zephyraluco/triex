# Triex 本地文件索引

此 Tauri 2 插件在 Windows 上用当前用户权限扫描本地固定磁盘，并在 Triex 进程内搜索文件名和路径。首次使用开始扫描。每轮扫描完成后等待 60 秒再更新，刷新期间继续使用最近一轮完成的索引。索引在内存中，重启后重新建立。无法访问的目录及目录重解析点会跳过。此后端没有 Everything 安装或运行依赖。

## 查询示例

- `年报 pdf` 匹配文件名或目录名中同时包含两个词的条目。
- `"年度 报告" ext:pdf !草稿` 匹配包含完整短语的 PDF，并排除草稿。
- `folder: path:"C:\工作目录"` 限定目录结果及路径。
- `ext:pdf;docx` 匹配任意一种扩展名。

查询使用不区分大小写的字面匹配，支持引号、否定及 `path:`、`ext:`、`file:`、`folder:`。首次扫描耗时取决于目录数量和磁盘速度。扫描间隔内的变化会在下一轮反映。完整 Everything 查询语法、文件内容搜索和 NTFS USN 实时增量索引不在当前实现范围内。

## 从 Everything 反汇编得到的边界

原插件 x64 模块通过 `FindWindowA` 查找 `EVERYTHING_TASKBAR_NOTIFICATION`，并调用 `SendMessageA` 发送 `WM_COPYDATA` 搜索请求。结果由 Everything 应用发送给原生模块的回调窗口。Everything 1.4.1.1009 二进制导入了 `ConnectNamedPipe` 和 `DeviceIoControl`，官方文档将服务定义为 NTFS 索引与变更监控的权限组件，SDK 文档要求应用客户端正在运行。在该版本的反汇编中，`0x4A5143`、`0x4A51CA` 和 `0x4BBF7B` 分别调用 `DeviceIoControl` 获取 NTFS 卷信息、读取 MFT 记录和读取 USN 日志。服务代码包含命名管道字符串 `\\.\PIPE\Everything Service`，并在 `0x4BD58C` 调用 `ConnectNamedPipe`。这些证据确认了管道传输和磁盘读取接口，私有消息格式仍未确定。Triex 使用 Windows 公开磁盘枚举和目录遍历接口实现独立索引。 [官方服务说明](https://www.voidtools.com/support/everything/everything_service/) · [SDK](https://www.voidtools.com/support/everything/sdk/)

## 宿主 API

为兼容 Triex 现有前端，crate 名称和 IPC 命名空间保留 `tauri-plugin-everything` 与 `everything`。宿主注册 `tauri_plugin_everything::init()`，通过 `EverythingExt` 访问 `status()` 和 `search(request)`。IPC 权限 `everything:allow-status` 与 `everything:allow-search` 仍需显式授予。Triex 使用自身的受限桥接。搜索接受 `query`、`offset`、`limit` 和 `sort`，返回总数、下一页偏移和结果字段。
