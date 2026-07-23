# ClipVault

Windows 桌面剪贴板管理器。自动记录复制历史，支持文本 / 富文本 / 图片，全局快捷键快速粘贴。

基于 **Tauri 2 + Vue 3 + Rust + SQLite**。

## 功能

- 后台监听剪贴板（约 500ms 轮询；OS 序列号未变则跳过读取），内容去重（SHA-256；粘贴回写不会重复建条）
- 类型：文本、代码、链接、图片、文件路径（敏感是标记字段，不是独立类型）
- 图片落盘（PNG + 列表缩略图）；列表只带截断正文，富文本 HTML 预览按需拉取；导出为全文+HTML
- 富文本：保留 HTML；预览经消毒后渲染；可「原格式」或「纯文本」粘贴
- 粘贴：写回系统剪贴板（图片优先 PNG 格式）后，把焦点还给唤出前的应用并模拟 Ctrl+V
- 搜索（≥3 字走 FTS5；1–2 字走轻量匹配）、可叠加类型 / 收藏 / 标签；收藏、置顶、标签、回收站、批量操作
- 自动打标：新记录按规则匹配（内容类型 / 关键词）；内置默认规则可改，设置里可关（默认开）
- 窗口模式列表可排序：最新 / 最早 / 最近创建 / 使用最多（置顶仍优先）；列表虚拟滚动
- 敏感内容检测与自动过期；忽略应用列表
- 悬浮面板 / 窗口两种界面；记住上次窗口尺寸；圆角 / 不透明度可调；毛玻璃仅悬浮模式生效（窗口模式为性能自动关闭）；关闭默认进托盘
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
npm test                 # 前端 Vitest（Pinia store 冒烟测试）
npm run lint             # ESLint 检查 src（.ts + .vue）
npx tauri icon app-icon.png -o src-tauri/icons   # 从源图生成全套图标

cargo test --manifest-path src-tauri/Cargo.toml  # Rust 后端测试（17 个）
```

修改 Rust 代码（`src-tauri/src/*.rs`）后，运行 `cargo test --manifest-path src-tauri/Cargo.toml` 验证后端测试仍全部通过。

## 数据位置

| 内容 | 路径 |
|------|------|
| 数据库 | `%LOCALAPPDATA%\ClipVault\clipvault.db` |
| 图片原图 | `%LOCALAPPDATA%\ClipVault\media\` |
| 缩略图 | `%LOCALAPPDATA%\ClipVault\media\thumbs\` |
| 日志 | `%LOCALAPPDATA%\ClipVault\logs\` |

设置 → 统计会显示数据目录绝对路径与本地存储占用估算（`content_len` 汇总 + `media/` 目录缓存，不含完整 SQLite 索引开销）。

历史设置说明：

- **回收站保留天数**：只清理回收站中的过期条目
- **最大记录数**：超出时淘汰未收藏、未置顶的最旧记录
- **自动打标**：设置 → 标签；新复制内容按规则打标签（默认规则含「链接 / 部署 / 前端」）；可增删改规则或关闭；同内容再次复制（hash 去重）不会重打标
- **粘贴后自动关闭面板**：悬浮模式下粘贴后是否保持关闭（关闭则粘贴后重新打开）
- **毛玻璃效果**：仅悬浮模式生效；切换到独立窗口时自动关闭以降低合成开销
- **导出记录**：流式写出全文与 HTML（与列表截断无关），可作备份再导入

## 技术栈

| 层 | 技术 |
|----|------|
| UI | Vue 3、TypeScript、Vite、Pinia、DOMPurify |
| 桌面壳 | Tauri 2 |
| 后端 | Rust、arboard、rusqlite |
| 存储 | SQLite（WAL + FTS5 + 读写分离连接池）+ 本地 media 目录 |

实现要点（供维护者）：捕获与 PNG/SQLite 落库解耦；过期/保留清理在独立定时线程；列表 keyset 分页与虚拟滚动；粘贴在 `spawn_blocking` + 异步延时上完成。完整架构说明见 [CLAUDE.md](./CLAUDE.md)。前端 UI 审查与落地状态见 [docs/ui-design-review.md](./docs/ui-design-review.md)。

## 许可

当前仓库未声明开源许可证；私有项目（`package.json` 中 `"private": true`）。
