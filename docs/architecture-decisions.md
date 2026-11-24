# Triex 架构与实现取舍记录（ADR）

本文档记录 **Triex**（基于 Tauri v2 的 Windows 应用启动器，交互与视觉优先参考 [ZTools](https://github.com/ZToolsCenter/ZTools)）在技术选型、架构设计与实现迭代过程中的关键决策与取舍。

---

## 1. 桌面容器与插件运行时：Tauri v2 纯原生路线 vs Electron + Node Sidecar

* **背景**：参考对象 Rubick 与 ZTools 均基于 Electron（内置 Node.js 与 Chromium `BrowserView` / `WebContentsView`），支持丰富的旧版 uTools/Rubick 插件；而 Triex 目标是极低内存占用与毫秒级响应的 Windows 原生启动器。
* **备选方案**：
  1. **Tauri v2 + Node/Bun Sidecar**：打包精简 Node 运行时以兼容依赖 `fs`/`child_process` 的旧版 `preload.js` 插件。
  2. **Tauri v2 纯原生轻量化**：使用 Tauri v2 Multiwebview（Child Webview）承载插件 UI，通过 `plugin://` 协议隔离源，`initialization_script` 注入轻量 `window.triex` SDK，所有系统能力由 Rust 后端按 `plugin.json` 权限受控提供。
* **最终选择**：**方案 2（纯原生轻量化）**。
* **取舍理由**：
  * 放弃直接兼容旧版 Electron/Node `preload.js` 插件，换取极小的安装包体积、单进程极低内存占用及更清晰的权限沙箱边界。
  * 后续若确实需要无 UI 插件脚本能力，优先评估内嵌轻量运行时（如 QuickJS / Wasmtime）而非重型 Node 进程。

---

## 2. 仓库组织结构：扁平单仓 + Vite 双入口 vs 嵌套 Monorepo

* **背景**：项目已基于 Bun + Tauri v2 标准模板初始化为根目录 `src/` + `src-tauri/` 结构。
* **备选方案**：
  1. 重构为 `apps/desktop` + `packages/*` 嵌套式 Monorepo。
  2. 保留现有根目录扁平结构，前端通过 Vite 多入口拆分 `index.html`（主启动器）与 `settings.html`（设置与插件管理窗口）。
* **最终选择**：**方案 2（保留扁平单仓 + Vite 双入口）**。
* **取舍理由**：
  * 避免早期为不存在的多包引入不必要的目录嵌套与构建配置复杂度。
  * 通过双 HTML 入口隔离主启动器与设置窗口代码，确保启动器首屏加载极致轻量；未来需要独立发布插件 SDK 时，再利用 Bun Workspaces 在根目录增量扩展 `packages/plugin-sdk`。

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
  * SQLite 开启 WAL 模式后，使用 `plugin_kv(plugin_id, key, value_json, rev, updated_at)` 表即可胜任插件隔离 KV 存储，同时为后续实现剪贴板历史全文搜索（FTS5）与复杂条件筛选留足空间。
  * **演进边界**：若未来剪贴板图片缓存或高频插件写入造成明显写锁竞争，再将大对象/KV 缓存独立拆迁至 `redb`。

---

## 4. 搜索引擎：Rust `nucleo-matcher` + `pinyin` 预计算索引

* **背景**：中文 Windows 启动器必须支持应用原名、中文全拼（`weixin`）、中文首字母（`wx`）、英文缩写（`vsc`）以及常用优先排序。
* **最终选择**：在 Rust 后端维护内存索引，使用 `nucleo-matcher` + `pinyin` + 7 天半衰期 Frecency 排序。
* **关键取舍**：
  * **索引期预计算字符映射**：构建索引时同步生成 `full_char_map` 与 `abbr_char_map`，将拼音命中的下标准确反查回原中文字符下标，前端无需做任何正则或二次匹配，直接按 `matchIndices` 高亮原文。
  * **冷启动缓存 + 后台扫描**：启动瞬间先从 SQLite `apps_cache` 加载上次索引立即可搜，后台异步线程并行扫描开始菜单 `.lnk`、`shell:AppsFolder`（UWP/MSIX）并由 `notify` 监听目录热更新。

---

## 5. 前端 UI、网格导航与虚拟列表取舍

* **技术选型**：React 19 + TypeScript + Tailwind CSS v4 + shadcn/ui + `cmdk` + Zustand。
* **关键取舍**：
  1. **关闭 `cmdk` 前端过滤**：设置 `<Command shouldFilter={false}>`，避免 JS 与 Rust 重复过滤冲突；搜索、打分与排序完全由 Rust 后端负责，前端使用递增 `requestId` 防止异步 IPC 响应乱序覆盖。
  2. **对齐 ZTools 9 列图标网格与 2D 导航**：放弃传统 1D 列表，采用 `800px` 宽 9 列卡片网格；在捕获阶段拦截 `← / → / ↑ / ↓`，按跨分组真实行列坐标计算焦点移动，覆盖 `cmdk` 默认的 1D 上下导航。
  3. **主列表不引入虚拟列表**：Rust 侧通过 Top-K 截断控制条目数，前端 DOM 节点少，渲染耗时 `< 1ms`；引入虚拟列表反而会破坏 `cmdk` 的 DOM 扫描与滚动跟随。仅在未来开发海量浏览型视图（如 Everything 全盘文件搜索、数千条剪贴板历史）时再局部引入 `@tanstack/react-virtual`。
  4. **极简状态与分组独立折叠**：
     * 不设常驻底部状态栏，搜索无结果时直接收起为 `64px` 单行搜索条，不展示「未找到」占位页。
     * 「最近使用」严格只展示存在真实启动记录（`frecency > 0`）的项目，首次启动绝不伪造未用过的应用填充。
     * 每个分组通过 `expandedSections: Record<string, boolean>` 独立控制超过 2 行（18 项）时的「展开全部 / 收起」，与 `Ctrl+M` 切换完整工作台解耦。

---

## 6. 前后端 IPC 契约：手写强类型封装 vs `tauri-specta`

* **背景**：`tauri-specta` 可从 Rust 自动生成 TypeScript 绑定，但目前对应 Tauri v2 的版本仍处于 RC 阶段，与 `specta` 宏版本耦合较紧。
* **最终选择**：现阶段采用 Rust `#[serde(rename_all = "camelCase")]` + 前端集中式强类型封装 [src/lib/ipc.ts](file:///d:/WorkSpace/triex/src/lib/ipc.ts)。
* **取舍理由**：以极少量手动同步成本换取零编译期宏冲突与稳定快速的依赖升级；待 `tauri-specta` v2 正式版稳定后可平滑替换。

---

## 7. 应用图标管线：`IShellItemImageFactory` + PNG 缓存 + `icon://` 协议

* **背景**：Windows 同时存在传统 Win32 `.lnk`/`.exe` 和微软商店 UWP/MSIX 应用，传统 `ExtractIconEx` 难以获取高清 UWP 图标。
* **最终选择**：统一使用 Windows Shell `IShellItemImageFactory`（UWP 通过 `shell:AppsFolder\<AUMID>` 路径接入）提取 64×64 RGBA 位图，经 `image` 编码为 PNG 缓存于 `%LocalAppData%\triex\icons\`，前端通过 Tauri v2 自定义协议 `http://icon.localhost/<hash>.png` 流式加载。
* **取舍理由**：避免将图标 Base64 塞入 JSON IPC 造成内存与序列化抖动，同时享受 WebView2 原生图片解码与缓存。

---

## 8. 右键上下文菜单：DOM 浮层 → 原生弹出菜单 → 独立透明窗体（演进中）

* **问题根因**：启动器主窗口按内容动态伸缩，仅有 1 行结果时总高仅约 `180px`；HTML `div` 菜单无法突破 WebView 窗口物理边界，必定会被底部裁切遮挡。
* **各方案对比**：
  * **DOM 浮层菜单**：样式自由，但在小高度窗体下必然被裁切（已淘汰）。
  * **Windows 原生弹出菜单（Tauri `window.popup_menu` / `muda`）**：底层调用 Win32 `TrackPopupMenuEx`，可自由超出主窗体边界且零额外内存开销；配合 `AppState.context_menu_open` 标志位防止弹出期间误触发失焦隐藏，并通过 Win32 剪贴板 API 直接实现路径复制。缺点是视觉为传统 Win32 风格，不如 Electron 基于 Chromium Views 自绘的现代大圆角阴影菜单美观。
  * **预热式独立透明弹出窗口（Popup `WebviewWindow`）**：在后台常驻一个隐藏的无边框透明小窗口，右键时定位到鼠标坐标展示 React + Tailwind 菜单。既可自由超出主窗体不被裁切，又能 1:1 复刻 ZTools 的现代大圆角白底/暗色阴影菜单。
* **当前状态与规划**：当前已落地 **原生弹出菜单** 彻底解决遮挡与功能可用性问题；后续视觉打磨阶段优先升级为 **独立透明弹出窗口** 以对齐 ZTools 现代圆角菜单体验。
