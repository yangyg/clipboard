# Clipboard

Windows 桌面剪贴板管理器。自动记录复制历史，支持文本 / 富文本 / 图片，全局快捷键快速粘贴。

基于 **Tauri 2 + Vue 3 + Rust + SQLite**。

## 功能

- 后台监听剪贴板（约 500ms 轮询；OS 序列号未变则跳过读取），内容去重（SHA-256；粘贴回写不会重复建条）
- 类型：文本、代码、链接、图片、文件路径（敏感是标记字段，不是独立类型）；链接含网页 URL 与下载协议（`magnet:` / `ed2k://` / `thunder://` / `ftp://`，整段识别），预览可点开系统默认程序；整段 CSS 色值仍属文本，列表/详情会显示色块预览
- 图片落盘（PNG + 列表缩略图）；列表只带截断正文，富文本 HTML 预览按需拉取；导出为全文+HTML
- 富文本：保留 HTML；预览经消毒后渲染；可「原格式」或「纯文本」粘贴
- 粘贴：写回系统剪贴板（图片优先 PNG 格式）后，把焦点还给唤出前的应用并模拟 Ctrl+V；开启「粘贴后自动关闭」时，悬浮模式隐藏、窗口模式最小化
- 搜索（≥3 字走 FTS5；1–2 字走轻量匹配，含别名）、可叠加类型 / 收藏 / 标签；收藏、置顶、短别名、标签、回收站、批量操作
- 自动打标：新记录按规则匹配（内容类型 / 关键词）；内置默认规则可改，设置里可关（默认开）；侧栏空标签收在「更多」
- 窗口模式列表可排序：最新 / 最早 / 最近创建 / 粘贴最多（置顶仍优先）；列表虚拟滚动（Fluent 扁平行）；列表 / 网格双视图，网格列数随宽度自适应（窄窗口自动降为 1 列）
- 敏感内容检测与自动过期；忽略应用列表
- 悬浮面板 / 窗口两种界面；记住上次窗口尺寸；三栏布局可拖拽调整宽度（侧边栏 / 列表 / 预览，自动记住）；圆角 / 不透明度可调；毛玻璃默认关，两种模式均可开启；关闭默认进托盘
- 界面字体可选：在「外观 → 界面字体」中切换 默认 / 微软雅黑 / 黑体 / 宋体 / 楷体 / Segoe UI 六个预设，或从系统已安装的中文字体中挑选（首次加载约一秒）；缺字自动回退到系统中文字体，字号单独调节
- 自定义托盘右键菜单（主题一致）；左键在后台时置前窗口、已前台时隐藏；休眠唤醒后自动恢复托盘与 WebView
- 首次安装显示轻量欢迎引导（快捷键 / 粘贴 / 托盘）；升级老用户不弹
- 单实例；可选开机自启；快捷键可在设置中修改并立即生效
- WebDAV 云同步（拉取 / 合并 / 推送；含图片媒体）；支持测试连接、手动同步
- 界面强调色为 Fluent 蓝 `#0078D4`；侧栏略抬升，列表与详情共用内容区底色

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
npm test                 # 前端 Vitest（组件 / store / utils，jsdom）
npm run lint             # ESLint 检查 src（.ts + .vue）
npm run doctor           # 环境诊断：Node / Rust / WebView2 / SQLite，异常时给出修复建议
npm run clippy           # Rust clippy 检查（-D warnings，与 CI 一致）
npm run typecheck        # vue-tsc --noEmit
npm run check:schema     # SQLite 建表 / 迁移一致性校验
npm run check:ipc-contract  # Rust 命令签名 ↔ TS 契约校验
npm run validate         # 本地全量校验（lint + typecheck + check:* + test + clippy + cargo-test）

cargo test --manifest-path src-tauri/Cargo.toml  # Rust 后端测试
```

修改 Rust 代码（`src-tauri/src/*.rs`）后，运行 `cargo test --manifest-path src-tauri/Cargo.toml` 验证后端测试仍全部通过。

CI（`.github/workflows/ci.yml`）在 push / PR 时自动运行前端 lint、类型检查、Vitest、`check:schema`、`check:ipc-contract`，以及 Rust clippy（`-D warnings`）、`cargo fmt --check` 与测试；Rust 侧在 `windows-latest` 上执行（代码依赖 Windows API）。

## 数据位置

| 内容 | 路径 |
|------|------|
| 数据库 | `%LOCALAPPDATA%\ClipVault\clipvault.db` |
| 图片原图 | `%LOCALAPPDATA%\ClipVault\media\` |
| 缩略图 | `%LOCALAPPDATA%\ClipVault\media\thumbs\` |
| 日志 | `%LOCALAPPDATA%\ClipVault\logs\` |

设置 → 数据会显示数据目录绝对路径与本地存储占用估算（`content_len` 汇总 + `media/` 目录缓存，不含完整 SQLite 索引开销）。

历史设置说明：

- **回收站保留天数**：只清理回收站中的过期条目
- **最大记录数**：超出时淘汰未收藏、未置顶的最旧记录
- **自动打标**：设置 → 标签；新复制内容按规则打标签（默认规则含「链接 / 部署 / 前端」）；可增删改规则或关闭；同内容再次复制（hash 去重）不会重打标
- **粘贴后自动关闭面板**：开启时悬浮模式粘贴后隐藏、窗口模式最小化；关闭则粘贴后恢复面板（不抢焦点）
- **毛玻璃效果**：默认关闭；开启后悬浮面板与独立窗口均生效，对窗口背后的内容做背景模糊
- **首次引导**：新安装弹出欢迎页；设置项 `onboarding_completed`；升级用户缺字段视为已完成
- **导出记录**：流式写出全文与 HTML（与列表截断无关），可作备份再导入

## 技术栈

| 层 | 技术 |
|----|------|
| UI | Vue 3、TypeScript、Vite、Pinia、DOMPurify |
| 桌面壳 | Tauri 2 |
| 后端 | Rust、arboard、rusqlite |
| 存储 | SQLite（WAL + FTS5 + 读写分离连接池）+ 本地 media 目录 |

实现要点（供维护者）：捕获与 PNG/SQLite 落库解耦；过期/保留清理在独立定时线程；列表 keyset 分页与虚拟滚动；粘贴写剪贴板后焦点还原 + Ctrl+V。架构决策记录（虚拟化引擎抽取、响应式网格列数等）见 [docs/adr/](./docs/adr/)；交互动效见 [docs/Clipboard-交互动效规范.md](./docs/Clipboard-交互动效规范.md)。

## 许可

当前仓库未声明开源许可证；私有项目（`package.json` 中 `"private": true`）。
