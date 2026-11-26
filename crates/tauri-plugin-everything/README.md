# Everything Windows 服务接入

Triex 连接已安装的 Everything Windows 服务，使用 Windows 用户权限运行。搜索流程无需启动 Everything 桌面进程。默认数据库是 `%APPDATA%\Everything\Everything.db`，也可用 `TRIEX_EVERYTHING_DB` 指定完整路径。

## 索引流程

- **有库时续读。** 读取 NTFS 快照，用卷 GUID、Journal ID 和 USN 检查点验证当前位置。通过服务命令 `0x12` 读取变更，在内存应用文件新增、删除、重命名、目录移动和文件元数据变化。
- **无库或检查点失效时重建。** 通过服务命令 `0x13`、`0x14`、`0x15` 枚举本地固定 NTFS 卷的 MFT，按文件引用号重建目录树。记录枚举开始前的 USN，再补读枚举期间的变化。遇到单条记录无法确定全部硬链接时也会走重建路径。
- **保存兼容数据库。** 按 `0x01070014` 格式和 `0x2505` 布局编码。保存会先写同目录临时文件。首次替换既有数据库前创建 `Everything.db.triex-backup`，检测到磁盘文件状态已变化时中止该次替换。保存自身引起的 USN 变更不会再次触发写库。

当前实施范围为 Everything 1.4.1.1032 的服务协议 8、已验证的 NTFS 数据库布局及 USN_RECORD_V2。启用了不同索引布局的数据库可能需要单独接入。现有查询支持名称、引号、否定和 `path:`、`ext:`、`file:`、`folder:`。完整 Everything 查询语法和同一 NTFS 卷上的端到端续读结果仍需对照验证。

## 验证与宿主接口

Rust 测试包括来自原版的真实 NTFS 快照、编码后读回、命名管道消息、USN 解码与回放、MFT 记录解码。Everything 1.4.1.1032 在 Wine 中加载 Triex 写出的 NTFS 快照，原版日志显示加载了 275,181 个目录和 1,295,711 个文件。Wine 测试卷没有与快照相匹配的活动 NTFS 日志，原版加载后转为重建。真实 Windows 的 MFT 枚举和日志续读需要另行验证。

宿主注册 `tauri_plugin_everything::init()`，通过 `EverythingExt` 访问 `status()` 和 `search(request)`。权限 `everything:allow-status` 和 `everything:allow-search` 仍由宿主显式授予。
