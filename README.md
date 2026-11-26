<div align="center">

# Triex

**轻量、极速、可扩展的 Windows 桌面应用启动器与插件平台**

基于 Tauri v2 + Rust + React 构建，交互与多视图架构参考 [ZTools](https://github.com/ZToolsCenter/ZTools)

![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11-0078D6?style=flat-square&logo=windows)
![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?style=flat-square&logo=tauri&logoColor=black)
![Rust](https://img.shields.io/badge/Rust-2024-DEA584?style=flat-square&logo=rust&logoColor=black)
![React](https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react&logoColor=black)
![Tailwind CSS](https://img.shields.io/badge/Tailwind%20CSS-v4-38B2AC?style=flat-square&logo=tailwind-css&logoColor=white)

</div>

---

## ✨ 核心特性

- **⚡ 毫秒级模糊检索与冷启动**
  - 纯 Rust 内存搜索引擎（`nucleo-matcher` + `pinyin`），支持中文原名、全拼（`weixin`）、拼音首字母（`wx`）、英文缩写（`vsc`）与别名匹配，并精确高亮原文命中字符。
  - 结合 7 天指数半衰期 Frecency 排序与常用固定项；启动瞬间直接加载 SQLite 缓存立即可搜，后台异步扫描并预热图标。
- **🪟 Windows 11 原生视觉融合**
  - 无边框透明窗体，强制开启 Windows 11 DWM 原生硬件圆角（`DWMWCP_ROUND`），支持 **Mica（云母）/ Acrylic（亚克力）/ 纯色** 材质切换与浅色 / 深色 / 跟随系统主题实时联动。
  - 多显示器自动跟随鼠标所在屏定位，内置离屏预重置机制彻底消除 WebView2 重开闪帧。
  - 自动索引开始菜单 `.lnk` / `.exe`、UWP / MSIX 应用（`shell:AppsFolder`）与常用系统指令，并通过 `notify` 监听目录热更新。
- **⌨️ 键盘优先的 2D 网格交互**
  - `800px` 宽 9 列图标网格，支持 `Tab` / `Shift+Tab` 与 2D 方向键无缝导航，鼠标悬浮与键盘选中视觉分层互不干扰。
  - 全局禁用浏览器默认菜单，应用卡片与品牌按钮统一使用 Tauri v2 原生右键菜单，所有可编辑输入框支持**右键一键粘贴**。
- **🧩 多 WebView 保活与沙箱插件宿主**
  - 纯原生父窗口托管物理零重叠的主壳 WebView（`main`）与下方子 WebView（`settings-view:*` / `plugin-*`），彻底避免层级遮挡与抢焦。
  - 设置页与第三方插件支持**按需创建 + 隐藏保活复用**（插件空闲 60 秒自动回收内存），兼顾极低冷启动开销与二次打开秒响应。
  - 插件通过自定义 `plugin://` 沙箱协议加载，内置 `window.triex` SDK，支持顶栏子输入框联动、按键透传、命名空间隔离 SQLite KV 存储与一键分离独立窗口。

---

## ⌨️ 常用快捷键

| 快捷键 | 说明 |
| :--- | :--- |
| `Alt + Space` | 全局呼出 / 隐藏启动器（可在设置中自定义） |
| `Tab` / `Shift + Tab` | 主搜索态循环切换候选图标；插件态透传给当前插件 |
| `←` `→` `↑` `↓` | 2D 网格方向导航 |
| `Enter` | 启动当前选中应用 / 指令 / 插件 |
| `Alt + Enter` | 以管理员身份运行选中应用（Win32） |
| `Ctrl + M` | 展开 / 收起所有应用、插件与系统指令面板 |
| `Ctrl + K` | 打开当前选中项的原生上下文菜单 |
| `Esc` | 逐层返回（关闭设置/插件 → 清除附件 → 清空输入 → 收起面板 → 隐藏窗口） |
| `输入框右键` | 直接粘贴系统剪贴板文本 |

---

## 🛠️ 技术栈

- **桌面容器与后端**：[Tauri v2](https://v2.tauri.app/)、Rust（`nucleo-matcher`、`pinyin`、`rusqlite` WAL、`windows-rs`、`window-vibrancy`、`notify`、`image`）
- **前端界面**：React 19、TypeScript、Vite、Tailwind CSS v4、[shadcn/ui](https://ui.shadcn.com/)、`cmdk`、Zustand
- **包管理与构建**：[Bun](https://bun.sh/) / Cargo

---

## 🚀 快速开始

### 环境要求

- **操作系统**：Windows 10 / 11（推荐 Windows 11 以获得 Mica 材质与 DWM 原生圆角）
- **Rust 工具链**：Rust 1.88+（MSVC toolchain）
- **前端运行时**：[Bun](https://bun.sh/)（或 Node.js 20+）
- **系统依赖**：Microsoft Edge WebView2 Runtime（Windows 11 已预装）

### 从源码运行

```bash
# 1. 克隆仓库
git clone <your-repo-url>
cd triex

# 2. 安装前端依赖
bun install

# 3. 启动桌面端开发模式（支持前后端热重载）
bun run tauri dev
```

### 构建与校验

```bash
# 前端类型检查与生产构建
bun run build

# Rust 静态检查与单元测试
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml

# 打包 Windows 安装包
bun run tauri build
```

---

## 本地文件搜索

在 Windows 上输入 `find` 或「本地搜索」。此功能读取已有 `Everything.db`，并连接 Everything Windows 服务核对 NTFS 卷和 USN 检查点。检查通过后可搜索、排序、分页、打开和定位文件。 [当前兼容范围](crates/tauri-plugin-everything/README.md)

## 🧩 本地插件开发简介

本地插件存放于 `%LocalAppData%\com.zeal.triex\plugins\<plugin-id>\`，根目录需包含 `plugin.json` 与入口 HTML 文件（放入后在设置页点击「立即重建」或重启即可生效）：

```json
{
  "id": "quick-notes",
  "name": "随手便签",
  "version": "1.0.0",
  "description": "轻量本地便签示例插件",
  "entry": "index.html",
  "height": 480,
  "permissions": ["db"],
  "features": [
    {
      "code": "notes",
      "explain": "随手便签",
      "cmds": ["note", "notes", "bq", "便签"]
    }
  ]
}
```

插件页面加载时宿主会自动注入 `window.triex` SDK（同时提供 `window.__TRIEX_THEME__` 与 `window.__TRIEX_FEATURE_CODE__`）：

- `window.triex.onPluginEnter(cb)`：监听插件进入/复用事件
- `window.triex.setSubInput(cb, placeholder)`：绑定顶栏子输入框内容变化
- `window.triex.onSubInputKeyDown(cb)`：接收顶栏 `Tab` / `Shift+Tab` 按键事件
- `window.triex.db.get / put / remove`：读写该插件隔离的本地 SQLite KV 存储（需声明 `"db"` 权限）
- `window.triex.clipboard.getSnapshot()`：读取当前剪贴板文本摘要
- `window.triex.hideMainWindow()`：隐藏主启动器窗口

---

## 📁 项目结构

```text
triex/
├── docs/
│   ├── architecture.md             # 完整系统架构、时序图与 IPC 契约
│   └── architecture-decisions.md   # 关键架构取舍记录 (ADR)
├── index.html / settings.html      # Vite 双入口（主壳 WebView / 设置子 WebView）
├── src/                            # React 前端
│   ├── App.tsx / SettingsApp.tsx   # 主壳与设置子 WebView 根组件
│   ├── components/                 # ui / launcher / settings 模块化组件
│   ├── hooks/                      # 搜索、窗口自适应、快捷键、事件、右键粘贴、主题联动
│   ├── stores/                     # 解耦的 useLauncherStore 与 useSettingsStore
│   └── lib/                        # 强类型 Tauri IPC 封装与协议工具
└── src-tauri/                      # Rust 后端 (triex_lib)
    ├── capabilities/               # 宿主与插件最小权限声明
    ├── resources/plugin-bridge.js  # 注入插件子 WebView 的 SDK 桥接脚本
    └── src/
        ├── lib.rs / state.rs / config.rs / error.rs / tray.rs
        ├── commands/               # IPC 命令与协调层 (launcher / menu / search / settings / plugin / clipboard)
        ├── search/                 # nucleo-matcher + 拼音投影 + Frecency 内存引擎
        ├── plugins/                # 插件清单、保活池与宿主/插件权限守卫
        ├── platform/               # Win32 / COM / Shell 扫描、图标提取、窗口与剪贴板封装
        ├── protocol/               # icon:// 与 plugin:// 自定义协议
        └── storage/                # SQLite WAL 持久化与迁移脚本
```

更多底层设计细节、时序图与安全边界说明请参阅 [系统架构文档](./docs/architecture.md) 与 [架构取舍记录 (ADR)](./docs/architecture-decisions.md)。

---

## 🙏 致谢

- 交互形态与多视图设计参考了开源启动器 [ZTools](https://github.com/ZToolsCenter/ZTools)
- UI 基础组件基于 [shadcn/ui](https://ui.shadcn.com/) 与 [cmdk](https://cmdk.paco.me/)
- 桌面运行时基于 [Tauri v2](https://v2.tauri.app/)
