# 随包 Everything 搜索

Windows 后端使用随包 Everything.exe 和官方 Everything SDK，通过 IPC 执行查询，支持 Everything 原生查询语法、排序和分页。

首次查询或状态检查时，如果 SDK 尚未检测到 Everything，插件将可执行文件释放到 Triex 的本地用户数据目录 Everything 子目录，并用 -startup 启动。后续状态检查反映启动与索引进度。随包 Everything.ini 在首次释放时写入，后续保留用户配置，其中 show_tray_icon=0 隐藏托盘图标。配置来自用户提供的 uTools 插件，包含 run_as_admin=1，Windows 可能要求提升权限。

检测到现有 Everything 实例时，SDK 使用该实例。Everything 的数据库、索引更新和服务接入均交由 Everything.exe 管理。后台进程会在 Triex 退出后继续运行。

Everything.exe 和初始配置取自用户提供的 uTools 插件安装包，编译时嵌入插件。SDK 许可见 sdk/LICENSE。实际 Windows 启动、权限提示和索引结果仍需在 Windows 上验收。
