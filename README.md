# ClipVault（剪贴板管理）

Windows 桌面剪贴板管理器。自动记录复制历史，支持文本 / 富文本 / 图片，全局快捷键快速粘贴。

基于 **Tauri 2 + Vue 3 + Rust + SQLite**。

## 功能

- 后台监听剪贴板（约 500ms 轮询），内容去重（SHA-256）
- 类型：文本、代码、链接、图片、文件路径
- 图片落盘（PNG + 列表缩略图）；列表不加载富文本 HTML，预览按需拉取
- 富文本：保留 HTML，可「原格式」或「纯文本」粘贴
- 搜索（可叠加类型 / 收藏 / 标签）、收藏、置顶、标签、回收站、批量操作
- 敏感内容检测与自动过期；忽略应用列表
- 悬浮面板 / 窗口两种界面；关闭默认进托盘
- 单实例；可选开机自启；快捷键可在设置中修改并立即生效

默认唤出快捷键：`Ctrl+Shift+V`。

## 环境要求

- Windows 10 / 11
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install)（stable）
- WebView2（Win10/11 一般已预装）

## 开发

```bash
npm install
npm run tauri dev
```

其它命令：

```bash
npm run dev              # 仅前端（端口 1420）
npm run build            # 类型检查 + 前端构建
npm run tauri build      # 打包桌面应用
npx tauri icon app-icon.png -o src-tauri/icons   # 从源图生成全套图标
```

## 数据位置

| 内容 | 路径 |
|------|------|
| 数据库 | `%LOCALAPPDATA%\ClipVault\clipvault.db` |
| 图片原图 | `%LOCALAPPDATA%\ClipVault\media\` |
| 缩略图 | `%LOCALAPPDATA%\ClipVault\media\thumbs\` |
| 日志 | `%LOCALAPPDATA%\ClipVault\logs\` |

设置页「本地存储占用」= 数据库文本体积 + `media/` 目录。

## 技术栈

| 层 | 技术 |
|----|------|
| UI | Vue 3、TypeScript、Vite、Pinia |
| 桌面壳 | Tauri 2 |
| 后端 | Rust、arboard、rusqlite |
| 存储 | SQLite（WAL）+ 本地 media 目录 |

架构细节（数据流、敏感规则、asset protocol、剪贴板优先级等）见 [CLAUDE.md](./CLAUDE.md)。

## 许可

当前仓库未声明开源许可证；私有项目（`package.json` 中 `"private": true`）。
