# ClipVault 前端 UI 设计审查报告

> 审查范围：`src/` 下全部 Vue 组件、全局样式 `src/styles/main.css`、composables、`index.html`、`src-tauri/tauri.conf.json`
> 审查日期：2026-07-23
> 结论先行：整体设计令牌体系与交互框架搭得不错（三主题 token、虚拟滚动、Toast/Confirm/快捷键体系完整），主要问题集中在 **设计 token 落地不彻底（硬编码色值/双套样式并存）**、**可访问性（键盘可达性、对比度）**、**组件重复（对话框/右键菜单/批量栏各写了两份）** 三个方面。

## 落地状态（2026-07-23）

四批改进已全部落地。下文保留原始审查记录，供对照；实现以代码与 [CLAUDE.md](../CLAUDE.md) 为准。

| 批次 | 状态 | 摘要 |
|------|------|------|
| 第一批 | 已完成 | `--type-*` / 全局 `.badge-*` 统一；残留硬编码改 token；TagDialog error toast + Esc；导入导出 success/error；窗口模式 minWidth **760**（`window.rs`） |
| 第二批 | 已完成 | `BaseDialog` / `ContextMenu` / `BatchBar`；focus trap + 菜单键盘；`--text-*` type scale；设置页等 rem |
| 第三批 | 已完成 | tertiary 提亮；listbox/option；全局 `:focus-visible`；表单 aria-label；主题 radiogroup |
| 第四批 | 已完成 | 粘贴主按钮；≤720px 断点；窗口模式关 blur；品牌名统一 ClipVault |

**未做 / 仍可后续打磨：** `TYPE_LABELS` 仍有多处文案副本（P1-6）；全站 `<main>` / 标题层级（P4-5 余项）；Toast 悬停暂停（P2-4）；分栏拖拽（P3-3）。

问题优先级标记：高（影响可用性/正确性）｜中（体验受损）｜低（打磨项）

---

## 1. 整体视觉一致性

### 做得好的
- `main.css` 有完整的三主题设计令牌（dark/light/OLED），颜色、阴影、圆角、过渡曲线全部 token 化，`prefers-reduced-motion` 也有兜底。
- `panel-surface` 共享面板 chrome，图标经 `AppIcon`/`TypeIcon` 统一收口。

### 存在的问题

**🔴 P1-1 两套"类型色"并存，同一类型在不同界面颜色不同**
`main.css` 定义了 `--type-code: #34d399`（绿）、`--type-image: #fbbf24`（黄）、`--type-link: #6366f1`（紫），`SettingsWindow.vue` 的自动标签规则在用；但 `RecordList.vue:690-713` 与 `PreviewPane.vue:453-476` 硬编码了另一套：code `#7c5cfc`（紫）、image `#e87d3e`（橙）、link `#17a97b`（绿）。用户会在"设置里是绿色、列表里是紫色"的困惑中失去对颜色语义的信任。
→ 改进：列表/预览一律改用 `var(--type-*)` + `color-mix()` 生成底色，删除全部硬编码类型色。

**🔴 P1-2 `.badge-*` 样式重复定义且互相覆盖**
`main.css:274-293` 全局定义了一套 badge（token 色），`RecordList.vue:758-786` 又在 scoped 里重写了一整套（硬编码色），scoped 胜出 → 全局那套实际无效，改 token 不会生效。
→ 改进：只保留全局一份，组件内删除。

**🟡 P1-3 硬编码色值偏离 token**
- `RecordList.vue:622` 选中边框 `rgba(79,110,247,0.2)` —— 这是旧版 accent（#4f6ef7 系），现 accent 已是 #6366f1，色调不一致的"残留色"。
- `PreviewPane.vue:553,823,832,841` 使用 `rgba(242,85,85,…)`、`rgba(245,166,35,…)`（即 #f25555/#f5a623），与 token `--danger: #f87171`、`--warning: #fbbf24` 均不同。
- `WindowControls.vue:110` 关闭按钮 hover `#e81123` 硬编码（遵循 Windows 惯例可接受，建议提为 `--win-close-hover` token）。

**🟡 P1-4 字体单位混用，字号缩放设置只生效一半**
主界面用 rem（随 `font_size` 设置缩放），但 `SettingsWindow.vue`、`ConfirmDialog.vue`、`TagDialog.vue`、`CaptureStatus.vue` 全部用 px（12.5px/11.5px/11px…）。用户调大"界面字体大小"后设置页纹丝不动。
→ 改进：统一 rem。

**🟡 P1-5 字号层级过碎，缺少 type scale**
现存 0.5625 / 0.563 / 0.625 / 0.656 / 0.688 / 0.69 / 0.719 / 0.72 / 0.75 / 0.78 / 0.81 / 0.813 / 0.85 / 0.875rem 共 14+ 个层级。建议收敛为 6 级：10 / 11 / 12 / 13 / 14 / 16(px)，并设为 `--text-xs` 等 token。同理建议补 `--space-*` 间距 token（目前 padding 各组件自定：12/14、10/16、16/20 不一）。

**🟡 P1-6 类型文案三处不一致**
`TYPE_LABELS` 在 `RecordList.vue:214`（"文本"）、`PreviewPane.vue:248`（"纯文本/代码片段/文件路径"）、`SettingsWindow.vue`（又一份）重复且措辞不同。
→ 改进：抽到 `stores/clipboard.ts` 或 `types.ts` 单一出处。

**🔵 P1-7 品牌名不一致**：标题栏显示"剪贴板管理 v0.1.0"，产品名为 ClipVault，关于页又是"剪贴板管理"。建议标题栏用「ClipVault」或「ClipVault 剪贴板管理」。

---

## 2. 交互体验

### 做得好的
- 反馈体系完整：Toast（4 种类型 + aria-live）、Confirm 危险确认、搜索防抖 + spinner、首载 loading、**空状态文案分场景且有行动引导**（"清除搜索"链接、"复制任意内容即可开始使用"）。
- 快捷键体系健全：方向键导航、Enter 粘贴、Alt+V、Ctrl+D/T、Del、Esc 分层处理（清空搜索 → 退出批量 → 取消选择 → 关闭面板），`useClipboardHotkeys` 抽取得当。
- 悬浮模式失焦自动隐藏、双击粘贴、回收站独立 banner，流程顺畅。

### 存在的问题

**🔴 P2-1 右键菜单键盘完全不可达**
`RecordList.vue:93-132` 与 `SideBar.vue:101-114` 的菜单项是 `<div @click>`：无法 Tab 聚焦、无方向键选择、Esc 不能关闭（只能靠全局点击收起）、无 `role="menu"`。且定位用魔法数（`wrapper.height - 210`、`Math.min(e.offsetY, 300)`），窗口底部右键时菜单位置不可靠。

**🔴 P2-2 失败路径静默**
`TagDialog.vue:177-179,193-195`：`createTag`/`setRecordTags` 失败时 catch 注释写着 "keep dialog open for retry"，但**用户收不到任何失败提示**——点了"创建"毫无反应，像死按钮。`SettingsWindow` 导入失败也仅用 accent（紫）色 `status-line` 展示，与成功态无色彩区分（导出成功才用绿色）。
→ 改进：所有 catch 补 `toast(msg,'error')`；状态行加 `success`/`error` 两类样式。

**🟡 P2-3 TagDialog 无 Esc 关闭、两对话框均无焦点圈禁（focus trap）**
`ConfirmDialog` 有 Esc 处理，`TagDialog` 没有；两者 Tab 都能跑到对话框背后的元素上（`aria-modal` 只在 ConfirmDialog 有）。

**🟡 P2-4 按钮反馈不完整**
- 全局按钮几乎都没有 `:active` 按下态，只有 hover；`.btn-danger`（main.css:339）连 hover 都没有。
- 筛选/切换类目时列表区域无任何过渡或骨架屏，仅搜索框右侧一个小 spinner，长列表切换像"闪换"。
- Toast 固定 2800ms 消失，悬停不暂停，长错误信息读不完。

**🟡 P2-5 悬浮/窗口两种模式信息架构不一致**
悬浮模式用横向 filter tabs，窗口模式用侧边栏 + 排序下拉；批量栏在两处各写一份且类名不同（`danger` vs `danger-btn`，见第 5 节）。同一功能两种操作路径，学习成本翻倍。可以接受"紧凑/完整"差异，但批量操作、清空回收站等核心动作的位置和样式应保持一致。

**🔵 P2-6 `PreviewPane.vue:77` 外链 `<a target="_blank">` 缺 `rel="noopener noreferrer"`**（Tauri 里风险较低，仍建议补上）。

---

## 3. 响应式适配

> 说明：ClipVault 是 Tauri 桌面应用（窗口默认 640×620，`resizable: true`），手机/平板场景不适用。本维度按**窗口尺寸缩放**评估。

**🔴 P3-1 默认窗口宽度下三栏布局最小宽度超限，预览区会被裁剪**
`WindowApp` 窗口体 = SideBar `min-width:200px` + RecordList `min-width:240px` + PreviewPane `min-width:300px` = **740px > 默认窗口 640px**。三处 `overflow: hidden` 会把超出部分直接裁掉——窗口模式下右侧预览内容默认就是被切掉的。
→ 改进：给窗口模式设 `minWidth: 760`（tauri.conf.json 或 Rust 端 resize），或窄宽度时隐藏/折叠预览区。

**🟡 P3-2 全局无任何媒体查询 / 容器查询**
窗口收窄时：设置页 `settings-nav` 固定 180px 挤压内容区；`theme-cards` 固定 4 栏；预览区 `preview-actions` 固定 5 栏 grid。建议至少加两条断点（如 <720px 侧边栏收成图标条、预览区动作改 3 栏）。

**🟡 P3-3 记录列表 `max-width:400px` 与预览 `flex:1.5` 组合僵硬**
宽窗口下列表被限宽、预览占据大量空间；窄窗口又不够分。可考虑让用户拖拽分栏，或预览区也设 `max-width`。

**做得好的**：虚拟滚动按视口高度自适应；`--ui-font-scale` 整体缩放可缓解小窗可读性；`html,body overflow:hidden` 对桌面单窗口应用合理。

---

## 4. 可访问性（a11y）

### 做得好的
- `ConfirmDialog` 用 `role="alertdialog"` + `aria-modal` + `aria-labelledby/describedby`，Esc 可关。
- Toast `aria-live`（error 用 assertive）；SideBar `aria-current`/`aria-pressed`；toggle 有 `role="switch"`；图标统一 `aria-hidden`；预览图有 `alt="剪贴板图片"`。

### 存在的问题

**🔴 P4-1 关键文本对比度不达标（WCAG AA 需 4.5:1）**（实测计算值）
| 组合 | 对比度 | 判定 |
|---|---|---|
| `--text-tertiary #5c6078` on 暗底 `#0f1117` | ≈ 3.3:1 | ❌ 普通文本不达标，却用于 9–11px 的时间/计数/说明文字 |
| `--text-tertiary #9498ae` on 亮底 `#ffffff` | ≈ 2.8:1 | ❌ 严重不达标 |
| OLED 主题 tertiary `#4a4a68` on `#000` | ≈ 2.5:1 | ❌ 严重不达标 |
| `--accent #6366f1` on 面板 `#181a22`（badge 小字） | ≈ 4.1:1 | ⚠️ 临界不达标 |
→ 改进：三主题 tertiary 全部提亮（如暗色 → #7a7f96，亮色 → #6b7089，OLED → #6a6a88）；9px 级文本直接弃用，最小字号提到 10–11px。

**🔴 P4-2 记录列表无语义、不可聚焦**
`record-item` 是 `<div @click>`：无 `tabindex`、无 `role`、无 `aria-selected`。虽有全局方向键导航，但焦点不可见（无 focus ring），读屏软件无法把它当列表朗读。
→ 改进：列表容器 `role="listbox"`、行 `role="option"` + `aria-selected` + `tabindex="-1"`（roving tabindex），配 `:focus-visible` 样式。

**🟡 P4-3 表单控件标签关联缺失**
- `SearchBar` 输入框只有 placeholder，无 `aria-label`；`SettingsWindow` 的忽略应用输入框同理。
- `TagDialog.vue:15` 的 `<label class="field-label">标签名称</label>` 与 input 是**兄弟节点**，既未包裹也无 `for` → 关联失败。
- 三个 `input[type=range]` 滑杆无 `aria-label`。
- 全局缺统一 `:focus-visible` 焦点环（目前仅 toggle、pin/star 有），按钮键盘聚焦时不可见。

**🟡 P4-4 主题卡片等 div 按钮不可键盘操作**
`SettingsWindow.vue` 主题选择卡片 `<div @click>` 无 `tabindex`/`role="radio"`；右键菜单同 P2-1。

**🔵 P4-5 其余小问题**：`SettingsWindow` 关于页 logo `alt=""`（品牌图建议给 alt）；`--sensitive` 橙色用于 `badge-sensitive`，但 `RecordList.vue:420` 实际把敏感记录映射到 `badge-danger` 红色，`--sensitive*` token 无人使用；全站无任何 `<h1>-<h6>` 与 `<main>` 地标。

---

## 5. 组件复用性

### 做得好的
- composables 抽取清晰（useToast / useConfirm / useClipboardHotkeys / useBatchActions），图标统一入口，`panel-surface` 共享面板样式。

### 存在的问题（重复代码清单）

**🔴 P5-1 对话框骨架整份复制**：`ConfirmDialog.vue` 与 `TagDialog.vue` 的 overlay / card / header / footer / btn-cancel / btn-confirm / modal transition 约 150 行样式一模一样。
→ 抽 `BaseDialog.vue`（含 focus trap + Esc + Teleport），两个对话框只做内容插槽。

**🔴 P5-2 右键菜单两份**：`RecordList.vue:921-975` 与 `SideBar.vue:432-479` 的 context-menu/ctx-item/ctx-sep 完全重复。
→ 抽 `ContextMenu.vue`（顺带解决 P2-1 键盘与边界定位问题，用 clamp 替代魔法数）。

**🟡 P5-3 批量操作栏两份**：`FloatingPanel.vue:258-311` 与 `WindowApp.vue:454-495` 的 batch-bar/batch-btn 各写一份，危险按钮类名还不同（`danger` vs `danger-btn`）——已经是"不一致的自定义样式"实锤。
→ 抽 `BatchBar.vue`。

**🟡 P5-4 其余重复**
- `.toggle` 在 `main.css:239`（全局）与 `SettingsWindow.vue` scoped 各定义一份。
- `TYPE_LABELS` × 3 处（见 P1-6）；`.badge-*` × 2 套（见 P1-2）；loading spinner 样式 × 2（RecordList 与 SearchBar 各自 keyframes spin）。
- 悬浮面板头部按钮组与窗口模式标题栏按钮组样式近似但未共用 `.icon-btn` 之外的抽象。

---

## 6. 性能优化

### 做得好的（这块是亮点）
- **自实现虚拟滚动**：`layoutSig` 只在 id/置顶变化时重建布局，内容字段变化不触发；滚动用 rAF 节流；`recordsById` 只对可见窗口 O(n) 查表。
- `getNow()` 30 秒缓存避免每行 new Date；图片列表用 thumb 优先 + asset 协议（不落 base64）；面板重载 30s TTL；搜索 250ms 防抖；规则提交 400ms 防抖；构建产物 JS 182KB / CSS 50.6KB，体积健康；lucide 图标具名导入可 tree-shake。

### 存在的问题

**🟡 P6-1 毛玻璃成本**：`.panel-surface` 在 `blur-enabled` 下常开 `backdrop-filter: blur(16px)`，叠加 `transparent: true` 的窗口，大窗口模式下是整个视口的持续合成开销。建议：窗口模式默认关 blur（悬浮小窗保留），或提供"性能模式"自动降级。

**🟡 P6-2 敏感内容倒计时每秒触发 PreviewPane 整组件重渲**（`expireTimer` 1s setInterval 改响应式 `expireNow`）。影响有限，可改为只更新倒计时文本节点。

**🔵 P6-3 资源**：`app-icon.png`（仓库根，422KB）若仅作打包源文件可接受；设置页用的 `src/assets/app-icon-128.png`（14.5KB）已压缩，OK。`dist` 已启用默认构建，无额外压缩/分包需求。

**🔵 P6-4 `html-preview` 的 `:deep(*) { max-width:100% !important; … }`** 对富文本预览是全子树样式重算，长 HTML 粘贴内容首次渲染会有可感知的 style recalc；可对 `content_html` 长度设上限或截断预览。

---

## 7. 视觉层次与信息架构

### 做得好的
- 三栏（导航 / 列表 / 预览）层级清晰，选中行有 accent 左侧指示条 + 底色双重编码；置顶分组有 section label。
- 敏感内容红色 banner + 自动删除倒计时，风险信息足够突出；空状态的引导文案分场景，信息架构友好。

### 存在的问题

**🟡 P7-1 核心动作"粘贴"视觉权重不足**
预览区底部 5 个等宽 `action-btn` 中，最高频的「粘贴」与「删除」权重相同（仅 hover 变色区分），删除还与收藏/置顶并列。建议：「粘贴」用 `btn-primary` 实心 accent 突出，「删除」降为图标按钮或移至右键菜单/快捷键。

**🟡 P7-2 记录行 meta 信息排版**
中文徽章加了 `text-transform: uppercase` + `letter-spacing: 0.03em`（英文排版习惯），中文带字间距显得松散怪异；时间、徽章、字符数三个 meta 信息字号相同，无主次。

**🟡 P7-3 窗口模式标题栏拥挤**
`titlebar` 38px 内塞标题+版本号+搜索框+窗口控制按钮，版本号胶囊（`titlebar-version`）信息价值低，建议移入关于页，把空间还给搜索框。

**🔵 P7-4 设置默认落点**：`activeSection` 默认 'appearance'，对首次用户合理；但"快捷键"这种高频诉求排第二，可考虑把"常规/快捷键"提前。

---

## 改进路线建议（按性价比排序）

> 下表为审查时规划；**四批均已落地**（见文首「落地状态」）。

| 批次 | 内容 | 对应问题 | 状态 |
|---|---|---|---|
| 第一批（1 天内可完成） | 删除全部硬编码色改 token、合并 `.badge-*` 双份定义、补全 catch 的 error toast、TagDialog 加 Esc、`minWidth` 修复 740px 裁剪 | P1-1~3, P2-2, P2-3, P3-1 | 已完成 |
| 第二批 | 抽 BaseDialog / ContextMenu / BatchBar 三组件，顺带补键盘导航与 focus trap；统一 rem 单位与 type scale | P5-1~3, P2-1, P1-4/5 | 已完成 |
| 第三批 | a11y 专项：提亮 tertiary 三色、listbox 语义、focus-visible 全局焦点环、label 关联、aria-label 补全 | P4-1~4 | 已完成 |
| 第四批（打磨） | 粘贴主按钮突出、两条窗口断点、blur 性能降级、品牌名统一 | P7-1, P3-2, P6-1, P1-7 | 已完成 |
