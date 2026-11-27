# Triex 架构与实现取舍记录（ADR）

本文档记录 **Triex**（基于 Tauri v2 的 Windows 应用启动器，交互与视觉综合参考 [ZTools](https://github.com/ZToolsCenter/ZTools) 与 [Rubick](https://github.com/rubickCenter/rubick)）在技术选型、架构设计与实现迭代过程中的关键决策与取舍。

---

## 1. 桌面容器与动态插件运行时：两档务实架构（Webview + 可选独立子进程）

* **背景**：参考对象 Rubick 与 ZTools 均基于 Electron（内置 Node.js 与 Chromium `BrowserView` / `WebContentsView`），支持丰富的旧版 uTools/Rubick 插件；而 Triex 目标是极低内存占用、毫秒级响应且安全可控的 Windows 原生启动器，同时需要支持既包含 Web UI 又包含本地后端服务（如文件索引、外挂工具、复杂计算）的动态插件。
* **备选方案**：
  1. **Tauri v2 + Node/Bun Sidecar**：打包精简 Node 运行时以兼容依赖 `fs`/`child_process` 的旧版 `preload.js` 插件。
  2. **仅纯 Webview 插件**：所有逻辑只能在前端运行，无法支撑多语言或重型本地服务。
  3. **Webview + 内嵌 QuickJS + 独立子进程三轨并行**。
  4. **两档混合架构：纯 Webview 插件 + 可选独立子进程后端（JSON-RPC 2.0 over stdio）**。
* **最终选择**：**方案 4（两档混合架构）**。
* **取舍理由**：
  * **90% 轻量 UI 插件**直接写在 Webview JS 中，通过 `window.triex.*` 调用 Rust 宿主提供的受控能力（`db`、`clipboard`、`process.exec`、`setHeight`），零额外运行时开销。
  * **10% 重型 / 多语言插件**通过 `plugin.json` 声明 `backend: { kind: "process" }`，由 `PluginBackendManager` 按需懒启动并经由 **stdio JSON-RPC 2.0** 桥接通信（不监听 TCP 端口，无端口冲突与防火墙弹窗，且由 Windows Job Object 自动清理防僵尸进程）。
  * **暂不引入 QuickJS**：在尚不需要主搜索框无界面实时推送（`mainPush`）的阶段避免过度设计；未来需要高频无界面搜索推送时可沿同一 `backend.kind` 枚举平滑扩展。

---

## 2. 仓库组织结构：扁平单仓 + Vite 双入口 vs 嵌套 Monorepo

* **背景**：项目已基于 Bun + Tauri v2 标准模板初始化为根目录 `src/` + `src-tauri/` 结构。
* **备选方案**：
  1. 重构为 `apps/desktop` + `packages/*` 嵌套式 Monorepo。
  2. 保留现有根目录扁平结构，前端通过 Vite 多入口拆分 `index.html`（主启动器）与 `settings.html`（设置与插件管理窗口）。
* **最终选择**：**方案 2（保留扁平单仓 + Vite 双入口）**。
* **取舍理由**：
  * 避免早期为不存在的多包引入不必要的目录嵌套与构建配置复杂度。
  * 通过双 HTML 入口隔离主启动器与设置窗口代码，确保启动器首屏加载极致轻量；参考示例插件放在根目录 `plugins/` 下（如 `plugins/calculator/`），可直接在设置页一键导入验证。

---

## 3. 本地持久化：单用 SQLite（WAL） vs SQLite + `redb` 双库

* **背景**：ZTools 使用 LMDB 作为高性能嵌入式 KV 存储；Rust 生态中 `redb` 是无 C 依赖的现代纯 Rust KV 替代品。初期方案曾规划 SQLite（索引/配置/历史）+ `redb`（插件 KV）双库。
* **备选方案**：
  1. SQLite + `redb` 双存储引擎并存。
  2. 单用 `redb`。
  3. **单用 SQLite（`rusqlite` bundled + WAL 模式）**。
* **最终选择**：**方案 3（暂不引入 `redb`，单用 SQLite）**。
* **取舍理由**：
  * 早期同时维护两套存储引擎收益极低，反而增加依赖体积、文件句柄与备份排查成本。
  * SQLite 开启 WAL 模式后，使用 `plugin_kv(plugin_id, key, value_json, rev, updated_at)` 表即可胜任插件隔离 KV 存储；独立进程后端插件另享有各自隔离的 `%LocalAppData%\triex\plugin-data\<id>` 文件目录。
  * **演进边界**：若未来剪贴板图片缓存或高频插件写入造成明显写锁竞争，再将大对象/KV 缓存独立拆迁至 `redb`。

---

## 4. 搜索引擎：从 `nucleo-matcher` 模糊子序列演进至连续子串 + 音节边界匹配

* **背景**：早期版本采用 `nucleo-matcher` 做模糊子序列匹配并按 `max_results = 30` 截断。实际使用中暴露出三个痛点：单字母/短词噪声过多、跨拼音音节误命中（如 `eix` 命中 `weixin`）、以及「展开全部」被 30 条上限截断导致名不副实；此外 UWP 自动注入 AUMID 内部包名关键词会导致 `z` 命中 `ZuneMusic`（媒体播放器）却无任何高亮。
* **最终选择**：参考 ZTools，全面切换为 **Rust 连续子串匹配 + 拼音音节边界对齐 + 无上限返回**，并移除 UWP 内部包名关键词注入。
* **关键取舍**：
  * **音节边界与半音节罚分**：索引期预计算 `full_is_start` / `full_is_end`、`name_compact` 与原字符映射表；全拼匹配要求起点对齐音节边界，停在半音节扣 `-30` 分，完整音节自然排在前面。
  * **分层打分与字符长度排序**：`exact (1050)` > `prefix (1000)` ≫ `contains (100)`，防止低相关中间匹配项凭高频反超前缀命中；次级排序使用 `chars().count()`（字符数）而非 UTF-8 字节数，避免中文吃亏。
  * **不限制返回个数**：连续子串本身大幅降低噪声命中，后端默认返回全部结果，前端折叠展示 2 行（18 项），点击「展开全部」可完整浏览全部匹配项。
  * **冷启动缓存 + 后台扫描**：启动瞬间先从 SQLite `apps_cache` 加载上次索引立即可搜，后台异步扫描 Win32/UWP/系统指令/插件并由 `notify` 监听热更新。

---

## 5. 前端 UI、网格导航、滚动条与顶栏布局取舍

* **技术选型**：React 19 + TypeScript + Tailwind CSS v4 + shadcn/ui + Zustand（**已彻底移除启动器中的 `cmdk` 包装层**）。
* **关键取舍**：
  1. **移除 `cmdk` 消除焦点闪烁**：`cmdk` 原生面向 1D 列表，即使关闭过滤与指针选择，其内部维护的 `data-selected` 与 `onValueChange` 仍与自定义 2D 网格状态冲突，导致切换搜索词时出现旧图标到首项的闪烁残影。改用纯 `div` 网格 + 单一 `selectedId` 状态源后，焦点切换瞬时、无残影。
  2. **0ms 即时搜索 + `resultQuery` 原子提交**：参考 ZTools，移除 `16ms` 搜索防抖；`useLauncherStore` 区分实时 `query` 与与结果绑定的 `resultQuery`，等待 IPC 返回期间保持旧分组与布局不动，结果返回时在同一次 `set()` 中原子提交 `results`、`resultQuery`、`expandedSections` 与默认首项 `selectedId`（后台 `index-updated` 或 `Ctrl+M` 切换工作台时保留当前选中）。
  3. **统一 Radix `ScrollArea` 悬浮滚动条**：为 `ScrollArea` 扩展 `viewportRef` 与 `viewportClassName`，主界面结果列表与设置页共用同一套悬浮滚动条，出现滚动条时占用 `0px` 布局宽度，彻底消除 9 列网格宽度抖动。
  4. **参考 Rubick 的顶栏左侧圆形图标与加载环**：主搜索态、设置态、插件态统一将圆形图标固定在最左侧、更多菜单（`⋮`）固定在最右侧；内置页面（搜索/设置）均保持主图标，进入外部插件时切换为插件自定义 `logo`（或默认紫色插件图标），并由后端 `PageLoadEvent::Finished`（`triex://view-loaded`）驱动外围旋转加载环准确起停。
  5. **主列表不引入虚拟列表**：网格按分组折叠展示（默认 2 行 18 项），DOM 节点极少；仅在未来开发海量浏览型视图（如全盘文件搜索、数千条剪贴板历史）时再局部引入 `@tanstack/react-virtual`。

---

## 6. 前后端 IPC 契约：手写强类型封装 vs `tauri-specta`

* **背景**：`tauri-specta` 可从 Rust 自动生成 TypeScript 绑定，但目前对应 Tauri v2 的版本仍处于 RC 阶段，与 `specta` 宏版本耦合较紧。
* **最终选择**：现阶段采用 Rust `#[serde(rename_all = "camelCase")]` + 前端集中式强类型封装 [src/lib/ipc.ts](file:///d:/WorkSpace/triex/src/lib/ipc.ts)。
* **取舍理由**：以极少量手动同步成本换取零编译期宏冲突与稳定快速的依赖升级；待 `tauri-specta` v2 正式版稳定后可平滑替换。

---

## 7. 应用与插件图标管线：`IShellItemImageFactory` + `icon://` / `plugin://` 双协议

* **背景**：Windows 同时存在传统 Win32 `.lnk`/`.exe`、UWP/MSIX 应用与第三方动态插件。
* **最终选择**：
  * 本地应用统一使用 Windows Shell `IShellItemImageFactory`（UWP 通过 `shell:AppsFolder\<AUMID>` 接入）提取 64×64 RGBA 位图，经 `image` 编码为 PNG 缓存于 `%LocalAppData%\triex\icons\`，前端通过 `http://icon.localhost/<hash>.png` 加载。
  * 动态插件通过 `plugin.json` 声明 `logo`，前端由 `getIconUrl` / `getPluginAssetUrl` 映射为 `http://plugin.localhost/<pluginId>/<logo>` 流式加载，无需二次转存。
* **取舍理由**：避免将图标 Base64 塞入 JSON IPC 造成内存与序列化抖动，同时享受 WebView2 原生图片解码与缓存。

---

## 8. 右键上下文菜单：DOM 浮层 → 原生弹出菜单 → 独立透明窗体（演进中）

* **问题根因**：启动器主窗口按内容动态伸缩，仅有 1 行结果时总高仅约 `180px`；HTML `div` 菜单无法突破 WebView 窗口物理边界，必定会被底部裁切遮挡。
* **各方案对比**：
  * **DOM 浮层菜单**：样式自由，但在小高度窗体下必然被裁切（已淘汰）。
  * **Windows 原生弹出菜单（Tauri `window.popup_menu` / `muda`）**：底层调用 Win32 `TrackPopupMenuEx`，可自由超出主窗体边界且零额外内存开销；配合 `AppState.context_menu_open` 标志位防止弹出期间误触发失焦隐藏，并通过 Win32 剪贴板 API 直接实现路径复制。缺点是视觉为传统 Win32 风格，不如 Electron 基于 Chromium Views 自绘的现代大圆角阴影菜单美观。
  * **预热式独立透明弹出窗口（Popup `WebviewWindow`）**：在后台常驻一个隐藏的无边框透明小窗口，右键时定位到鼠标坐标展示 React + Tailwind 菜单。既可自由超出主窗体不被裁切，又能 1:1 复刻 ZTools 的现代大圆角白底/暗色阴影菜单。
* **当前状态与规划**：当前已全量落地 **统一 Tauri v2 原生弹出菜单**（覆盖应用图标与品牌圆标菜单，并全局屏蔽 Edge WebView2 默认浏览器菜单），彻底解决矮窗口与 `64px` 顶栏裁切问题；后续若需进一步对齐现代大圆角视觉，可平滑演进为独立透明弹出小窗。
