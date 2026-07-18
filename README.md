# ClipVault（剪贴板管理）

Windows 桌面剪贴板管理器。自动记录复制历史，支持文本 / 富文本 / 图片，全局快捷键快速粘贴。

基于 **Tauri 2 + Vue 3 + Rust + SQLite**。

## 功能

- 后台监听剪贴板（约 500ms 轮询），自动去重（SHA-256）
- 内容类型：文本、代码、链接、图片、文件路径
- 图片落盘存储（PNG + 列表缩略图），不占用数据库大字段
- 富文本：保留 HTML，可「原格式」或「纯文本」粘贴
- 搜索、收藏、置顶、标签、回收站、批量操作
- 敏感内容检测（密码关键词、验证码、`sk-` API Key、疑似银行卡号），可自动过期删除
- 悬浮面板 / 窗口两种界面；关闭窗口默认最小化到托盘
- 单实例运行；可选开机自启

默认唤出快捷键：`Ctrl+Shift+V`（可在设置中修改相关行为）。

## 环境要求

- Windows 10 / 11
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install)（stable）
- WebView2（Win10/11 一般已预装）

## 开发

```bash
# 安装依赖
npm install

# 启动（Vite + Rust / Tauri）
npm run tauri dev
```

其它命令：

```bash
npm run dev       # 仅前端（端口 1420）
npm run build     # 类型检查 + 前端构建
npm run tauri build   # 打包桌面应用
```

## 数据位置

| 内容 | 路径 |
|------|------|
| 数据库 | `%LOCALAPPDATA%\ClipVault\clipvault.db` |
| 图片原图 | `%LOCALAPPDATA%\ClipVault\media\` |
| 缩略图 | `%LOCALAPPDATA%\ClipVault\media\thumbs\` |
| 日志 | `%LOCALAPPDATA%\ClipVault\logs\` |

## 技术栈

| 层 | 技术 |
|----|------|
| UI | Vue 3、TypeScript、Vite、Pinia |
| 桌面壳 | Tauri 2 |
| 后端 | Rust、arboard、rusqlite |
| 存储 | SQLite（WAL）+ 本地 media 目录 |

更细的架构说明（数据流、模块划分、敏感规则、asset protocol 等）见 [CLAUDE.md](./CLAUDE.md)。

## 许可

当前仓库未声明开源许可证；私有项目（`package.json` 中 `"private": true`）。
