# Everything Windows 服务接入

此 Tauri 2 插件从本机 `Everything.db` 加载文件名索引，在 Windows 用户会话中通过默认命名管道连接 Everything 服务。Triex 不启动 Everything 桌面进程。

此版本支持 Everything 1.4.1.1032 上已确认的 NTFS 快照配置，文件格式版本为 `0x01070014`，索引标志为 `0x2505`，服务协议版本为 8。默认读取 `%APPDATA%\Everything\Everything.db`。测试或非默认安装可用 `TRIEX_EVERYTHING_DB` 指定数据库的完整路径。

Triex 只读加载数据库并重建目录路径。读取成功后，向服务检查协议版本、每个 NTFS 卷和当前 USN 日志。只有卷映射、日志标识和快照位置都匹配时才发布搜索结果。快照需要补读日志、服务不可用或数据库格式不匹配时，搜索界面会显示对应状态。

目前仅实现快照读取和服务状态验证。USN 增量回放、通过 MFT 首次建库，以及兼容的 `Everything.db` 写入均待实现。快照过期时当前实现会明确报错。

## 查询与接口

支持不区分大小写的名称匹配、引号、否定，以及 `path:`、`ext:`、`file:`、`folder:`。示例为 `"年度 报告" ext:pdf !草稿`。排序、分页、打开与定位继续使用 Triex 原有的 IPC 通道和权限检查。

宿主注册 `tauri_plugin_everything::init()`，可通过 `EverythingExt` 使用 `status()` 和 `search(request)`。Tauri 权限 `everything:allow-status` 和 `everything:allow-search` 需由宿主显式授予。
