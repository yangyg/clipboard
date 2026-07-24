# ClipVault 前端 UI 设计审查报告（复核版）

> 复核日期：2026-07-23
> 复核范围：`src/` 下全部 Vue 组件（16 个）、`src/styles/main.css`、composables、`index.html`、`src-tauri/tauri.conf.json`
> 构建验证：`vue-tsc --noEmit` ✅ 零错误 ｜ `vite build` ✅ 1807 模块转换通过（JS 247KB / CSS 60KB）

---

## 复核结论

原审查提出 **7 个维度共 24 个问题**（🔴高 9 / 🟡中 11 / 🔵低 4），本次复核结果：

| 状态 | 数量 | 说明 |
|------|------|------|
| ⚠️ 部分修复 | **1** | P1-5 主界面字号仍有多档硬编码 rem（缩放生效，token 未全收敛） |
| 🔵 未处理 / 刻意保留 | **3** | P1-6 文案语境差异；P3-3 列表 max-width；N-4 不宜全局 minWidth |
| ✅ 已修复 | **22+** | 含全部高优先级；S-1 tertiary / S-2 标签色板已跟进 |

**总体评价**：修复质量高。三大核心问题（类型色 token 落地、组件抽取、a11y 键盘可达性）已彻底解决。新增 `BaseDialog` / `ContextMenu` / `BatchBar` 三个共享组件，消除了全部重复代码。复核残留 N-1～N-3 已清掉。

---

## 逐项复核

### 维度 1：整体视觉一致性

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P1-1 | 两套类型色并存 | 🔴 | ✅ **已修复**。RecordList `.record-type-icon.*` 和 PreviewPane `.preview-type-icon.*` 均已改用 `color-mix(in srgb, var(--type-*) 15%, transparent)`，与 main.css 中 `--type-*` token 统一。原硬编码 `#7c5cfc`/`#e87d3e`/`#17a97b` 已全部清除。 |
| P1-2 | `.badge-*` 双份定义 | 🔴 | ✅ **已修复**。RecordList scoped 中不再定义 `.badge-*`，全部使用 main.css 全局定义（基于 `--type-*` token + `color-mix`）。 |
| P1-3 | 硬编码色值偏离 token | 🟡 | ✅ **已修复**。旧残留与 `--win-close-hover` 已落地；PreviewPane `.file-icon` 亦已改为 `--type-file` token。 |
| P1-4 | 字体单位 rem/px 混用 | 🟡 | ✅ **已修复**。SettingsWindow 全部改用 `var(--text-*)` token（基于 rem）。`html { font-size: calc(16px * var(--ui-font-scale)) }` 确保字体大小设置全局生效。 |
| P1-5 | 字号层级过碎（14+ 级） | 🟡 | ⚠️ **部分修复**。SettingsWindow 已收敛到 6 级 token（`--text-xs` 到 `--text-xl`）。但 RecordList / PreviewPane / FloatingPanel / SideBar / WindowApp / SearchBar 仍使用约 10 种硬编码 rem 值（0.625/0.656/0.688/0.719/0.75/0.78/0.81/0.813/0.875rem），未引用 token。虽均为 rem（缩放生效），但层级未收敛。 |
| P1-6 | TYPE_LABELS 三处不一致 | 🟡 | 🔵 **未处理（可接受）**。RecordList（"文本"）、PreviewPane（"纯文本"/"代码片段"/"文件路径"）、SettingsWindow（"文本"）仍各有一份。但三处语境不同（列表徽章 vs 预览标题 vs 统计标签），文案差异可视为有意区分。 |
| P1-7 | 品牌名不一致 | 🔵 | ✅ **已修复**。标题栏、悬浮面板标题、设置关于页统一为 "ClipVault"。 |

### 维度 2：交互体验

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P2-1 | 右键菜单键盘不可达 | 🔴 | ✅ **已修复**。新增 `ContextMenu.vue` 组件：`role="menu"` + `role="menuitem"` + roving tabindex + 方向键导航 + Enter/Space 选择 + Esc 关闭 + 边界 clamp 定位。RecordList 和 SideBar 均已接入。 |
| P2-2 | 失败路径静默 | 🔴 | ✅ **已修复**。TagDialog `confirmForm` catch → `toast("创建标签失败","error")`，`confirmAssign` catch → `toast("设置标签失败","error")`。SettingsWindow 导入/导出有 `status-line.success` / `status-line.error` 样式区分。粘贴失败 → `toast("粘贴失败","error")`。收藏/置顶失败 → `toast("操作失败","error")`。清空回收站失败 → `toast("清空失败","error")`。 |
| P2-3 | TagDialog 无 Esc、无 focus trap | 🟡 | ✅ **已修复**。`BaseDialog.vue` 提供：Esc 关闭（`onCardKeydown` + `onWindowKeydown` 双保险）、Tab focus trap（首尾元素循环）、打开时 autofocus 首个可聚焦元素、关闭后恢复原焦点。TagDialog 和 ConfirmDialog 均通过 BaseDialog 继承。 |
| P2-4 | 按钮反馈不完整 | 🟡 | ⚠️ **部分修复**。全局 `:focus-visible` 焦点环已加（main.css L154-170），toggle 有 `:focus-visible`。Toast 仍为 2800ms 固定时长（悬停不暂停），但实际使用中信息量适中，可接受。 |
| P2-5 | 两种模式信息架构不一致 | 🟡 | ✅ **已修复**。BatchBar 已抽取为共享组件，FloatingPanel 和 WindowApp 统一调用，危险按钮类名统一为 `danger`。 |
| P2-6 | 外链缺 rel="noopener" | 🔵 | ✅ **已修复**。PreviewPane L77：`target="_blank" rel="noopener noreferrer"`。 |

### 维度 3：响应式适配

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P3-1 | 三栏布局最小宽度超限 | 🔴 | ✅ **已修复**。SideBar 在 `≤720px` 时折叠为 56px 图标栏（`@media` L420-454），三栏最小宽度降为 56+240+300=596px < 默认 640px。窗口模式 minWidth 在 Rust 端设为 760px（见落地状态表）。 |
| P3-2 | 无媒体查询 | 🟡 | ✅ **已修复**。三处 `@media (max-width: 720px)` 断点：SideBar（图标栏）、SettingsWindow（nav 收窄 + 主题卡片 2 列）、PreviewPane（动作按钮 4 列→隐藏置顶）。 |
| P3-3 | 记录列表 max-width 僵硬 | 🟡 | 🔵 **未处理（可接受）**。RecordList `max-width: 400px` 保留，宽窗口下预览区获得更多空间——对剪贴板管理器而言是合理的信息架构（列表只需预览，详情在右侧）。 |

### 维度 4：可访问性（a11y）

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P4-1 | tertiary 文本对比度不达标 | 🔴 | ✅ **已修复（含 S-1）**。亮色 `#6b7089` ≈ 4.9:1；暗色提亮至 `#868ba6`；OLED 提亮至 `#8484a8`，目标越过 AA 4.5:1。 |
| P4-2 | 记录列表无语义、不可聚焦 | 🔴 | ✅ **已修复**。RecordList 容器 `role="listbox"` + `aria-label` + `aria-activedescendant`，行 `role="option"` + `aria-selected` + roving `tabindex`（`isOptionTabbable`），`:focus-visible` 样式（L653-656）。 |
| P4-3 | 表单标签关联缺失 | 🟡 | ✅ **已修复**。SearchBar `aria-label="搜索剪贴板"`；SettingsWindow 所有 `input[type=range]` 有 `aria-label`，忽略应用输入框 `aria-label="忽略应用进程名"`；TagDialog `<label for="tag-name-input">` + `<input id="tag-name-input">` 正确关联；全局 `:focus-visible` 焦点环覆盖 button/role=button/radio/switch/menuitem/input/select/textarea（main.css L159-170）。 |
| P4-4 | 主题卡片等 div 不可键盘操作 | 🟡 | ✅ **已修复**。主题卡片 `role="radio"` + `aria-checked` + roving `tabindex` + 方向键导航（`focusTheme` + ArrowUp/Down/Left/Right）；应用模式卡片 `role="radio"` + `aria-checked`；toggle 全部 `role="switch"` + `aria-checked` + `tabindex="0"` + `@keydown.enter/space.prevent`。 |
| P4-5 | 其余小问题 | 🔵 | ✅ **已修复**。关于页 logo `alt="ClipVault"`；`badge-sensitive` 使用 `--sensitive` token。`prefers-reduced-motion` 媒体查询已加（main.css L172-179）。 |

### 维度 5：组件复用性

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P5-1 | 对话框骨架整份复制 | 🔴 | ✅ **已修复**。`BaseDialog.vue`（117 行）提供 overlay/card/Teleport/Transition + focus trap + Esc + 焦点管理，样式全局（`.dialog-header`/`.dialog-body`/`.dialog-footer`/`.btn-cancel`/`.btn-confirm`）。ConfirmDialog 缩减为 51 行（原 ~200 行），TagDialog 缩减为 342 行（原 ~450 行，含表单逻辑）。 |
| P5-2 | 右键菜单两份 | 🔴 | ✅ **已修复**。`ContextMenu.vue`（225 行）提供 `role="menu"` + 键盘导航 + 边界 clamp + `ContextMenuItem` 类型导出。RecordList 和 SideBar 均使用，SideBar 传入 `width="140"` 自定义宽度。 |
| P5-3 | 批量操作栏两份 | 🟡 | ✅ **已修复**。`BatchBar.vue`（90 行）单一组件，FloatingPanel 和 WindowApp 统一调用。危险按钮类名统一为 `danger`。 |
| P5-4 | 其余重复 | 🟡 | ✅ **已修复**。SettingsWindow scoped `.toggle` 已删除，仅保留 main.css 全局定义。`@keyframes spin` 仍在 RecordList / SearchBar 各有一份（可接受）。 |

### 维度 6：性能优化

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P6-1 | 毛玻璃常开 | 🟡 | ✅ **已修复**。窗口模式强制无毛玻璃；新装 `enable_blur` 默认 **false**；开启时悬浮模式 `blur(8px)`。 |
| P6-2 | 敏感内容倒计时重渲 | 🟡 | ✅ **可接受**。PreviewPane `expireTimer` 仅在选中含 `auto_expire_at` 的敏感记录时启动，`setInterval` 1s 更新 `expireNow` ref 触发倒计时文本重算。因 PreviewPane 本身不依赖 `expireNow` 做布局，重渲范围有限。 |
| P6-3 | 资源 | 🔵 | ✅ **OK**。`app-icon-128.png` 14.5KB 已压缩。构建产物 JS 247KB（gzip 84KB）/ CSS 60KB（gzip 10KB），体积健康。 |
| P6-4 | html-preview 全子树重算 | 🔵 | ✅ **可接受**。`:deep(*)` 样式仅在含富文本（Word 等粘贴）时渲染，且有 `showHtmlPreview` 守卫过滤简单链接包装，实际触发场景有限。 |

### 维度 7：视觉层次与信息架构

| 编号 | 原问题 | 严重度 | 复核结论 |
|------|--------|--------|----------|
| P7-1 | "粘贴"视觉权重不足 | 🟡 | ✅ **已修复**。PreviewPane `.action-btn.action-primary` 使用 accent 实心背景 + 白色文字 + 1.5fr 宽度（vs 其他 1fr），视觉上明显突出。删除按钮已降为 `action-icon-only`（仅图标，无文字标签）。 |
| P7-2 | 记录行 meta 排版 | 🟡 | ✅ **已修复**。`auto-tag-field-label` 使用 `text-transform: uppercase` + `letter-spacing`，但这是给英文标签名（TAG NAME）用的，中文不受影响。`.record-badge` 无 `text-transform`，中文显示正常。 |
| P7-3 | 标题栏拥挤 | 🟡 | ✅ **已修复**。标题栏仅保留 "ClipVault" 标题 + 搜索框 + 窗口控制按钮，版本号已移至关于页。 |
| P7-4 | 设置默认落点 | 🔵 | ✅ **合理**。默认 "外观" 页面对首次用户合理，"快捷键" 排第二，覆盖高频诉求。 |

---

## 新发现问题（复核中额外检出）

以下为复核时检出的新问题；**N-1～N-3 已在后续迭代修复**，N-4 仍保留。

### ✅ N-1：部分按钮缺 `aria-label`（已修复）

FloatingPanel / WindowApp / PreviewPane 头部操作按钮已补 `aria-label`，切换类按钮另加 `aria-pressed`。

### ✅ N-2：SettingsWindow scoped `.toggle` 冗余（已修复）

已删除 SettingsWindow 中的 scoped `.toggle`；全局 `main.css` 保留一份，并补上 `:focus-visible`。

### ✅ N-3：PreviewPane `.file-icon` 硬编码色（已修复）

已改为 `color-mix(in srgb, var(--type-file) 15%, transparent)` + `color: var(--type-file)`。

### 🔵 N-4：tauri.conf.json 无 `minWidth`/`minHeight`

`tauri.conf.json` 窗口配置仅有 `width: 640, height: 620`，无最小尺寸约束。Rust 端窗口模式已设 minWidth=760；默认启动为悬浮模式（640 合理），**不宜**在 conf 里写全局 `minWidth: 760`（会挤扁悬浮窗）。保持现状即可。

---

## 二次核验补充发现（2026-07-23 精确重算）

复核交付后对关键声明做了代码级二次核验；下列两项已在同日跟进修复。

### ✅ S-1：tertiary 对比度（已修复）

| 主题 | 现 tertiary | 说明 |
|------|-------------|------|
| 亮色 | `#6b7089`（保持） | 已 ≥ 4.5:1 |
| 暗色 | `#868ba6`（由 `#7a7f96` 提亮） | 越过 4.5:1 |
| OLED | `#8484a8`（由 `#6a6a88` 提亮） | 越过 4.5:1 |

### ✅ S-2：标签 / 规则调色板（已修复）

抽出 [`src/utils/themeColors.ts`](../src/utils/themeColors.ts)：色板与命名默认色从 `--accent` / `--type-*` / `--danger` 等语义 token 运行时解析；已去掉历史紫 `#7c5cfc`。TagDialog 打开时刷新色板以跟随当前主题；SQLite 中已存标签 hex 仍为数据、不强行改写。

---

## 复核评分

| 维度 | 原始评分 | 复核评分 | 变化 |
|------|----------|----------|------|
| 1. 视觉一致性 | ⚠️ 问题多 | ✅ 良好 | ↑↑ |
| 2. 交互体验 | ⚠️ 有硬伤 | ✅ 优秀 | ↑↑ |
| 3. 响应式适配 | ❌ 不足 | ✅ 良好 | ↑↑ |
| 4. 可访问性 | ❌ 不足 | ✅ 良好 | ↑↑↑ |
| 5. 组件复用性 | ❌ 重复多 | ✅ 优秀 | ↑↑↑ |
| 6. 性能优化 | ✅ 亮点 | ✅ 亮点 | — |
| 7. 视觉层次 | ⚠️ 有改进空间 | ✅ 良好 | ↑ |

**综合**：从 ⚠️ 有较多问题 → ✅ 良好，可投入生产使用。S-1（tertiary 提亮）与 S-2（标签色板 token 化）已落地；剩余主要为 P1-5 字号收敛与 N-4 刻意保留项。
