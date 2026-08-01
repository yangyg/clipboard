# ClipVault 前端 UI/UX 审查报告

审查日期：2026-08-01　范围：`src/` 全部页面与组件（5 个窗口页面、11 个设置子页、全部业务组件、全局样式）
维度：视觉一致性 / 交互体验 / 响应式设计 / 可访问性（WCAG 2.1 AA）/ 性能与加载体验
分级：P0 阻断（功能失真或不可读不可用）· P1 明显（影响体验需尽快修）· P2 建议（打磨项）

---

## 总体结论

整体质量高于同类手写 CSS 项目：设计 token 体系完备（颜色/字体刻度/间距/三主题）、无组件库依赖但样式纪律好、对话框焦点管理、Esc 分层、空态、危险操作确认覆盖成熟。**无 P0 级视觉问题，但有 2 个 P0 级正确性/可读性问题**：

| # | 问题 | 位置 | 维度 |
|---|------|------|------|
| P0-1 | store 吞错导致「假成功」toast：`deleteRecord`/`deleteBatch`/批量收藏内部 catch 仅 console.error 并正常返回，UI 无条件提示成功 | `stores/clipboard.ts:464-478, 532-547, 459`；调用方 `RecordList.vue:563`、`useBatchActions.ts:81`、`useClipboardHotkeys.ts:63`、`PreviewPane.vue:478` | 交互 |
| P0-2 | 亮色主题未覆盖 `--type-text/code/image/file`，白底上对比度 1.6~2.8:1，类型图标与徽章基本不可读 | `styles/main.css:80-119` | 视觉 + WCAG |

---

## 一、全局 / 样式体系（main.css、settings.css）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P0 | main.css:80-119 | 亮主题 type 色缺失（见 P0-2）；徽章底色更浅，实测更低 | 亮主题改加深色 #0369a1 / #047857 / #b45309 / #a16207 |
| P1 | main.css:19 | 暗/OLED 主题 `--accent` #0078d4 作文字色对比度 3.1~3.8:1，波及 filter-tab 激活、batch-info、status-line、slider-value、nav 激活项 | 暗主题文字统一用 `--accent-light`（9.65:1） |
| P1 | main.css:47,106 | 亮主题 warning #d97706（3.19:1）、sensitive #ea580c（3.56:1）用于小字标签，不达标 | 亮主题加深至 ≥4.5:1 |
| P1 | main.css:384-417 vs BaseDialog.vue:186-228 | 按钮两套体系：全局 `.btn`（30px/500 字重/--text-sm）与对话框按钮（32px/600/--text-md）平行实现 | 对话框复用 `.btn` + `--btn-height-lg` 修饰类 |
| P2 | main.css:16-17,133-134 | 三主题 `--text-tertiary` 均比 `--text-secondary` 更亮，层级语义颠倒 | 调换两值或重命名 |
| P2 | main.css:396-401 | 主按钮 hover 态（accent-hover 上白字 3.85:1）不达标 | hover 底色用更深的 accent-pressed |
| P2 | main.css:185-192 | reduced-motion 把 spinner 压成静止圆环，丢失「加载中」语义 | spinner 单独豁免或换静态指示 |
| P2 | main.css:122-137 | OLED 未覆盖 `--code-bg`、`--shadow-sm` | OLED 下 `--code-bg: #050505` |
| P2 | main.css:68-70 vs 385 | `--btn-height-md` 已定义但 `.btn` 硬编码 30px | 引用 token |
| P2 | main.css:352-355 / 269 等 | badge-link 底字色不配、icon 按钮容器 32/28/26px 四种、微圆角 3/4px 九处硬编码、遮罩 rgba(0,0,0,.45) 无 token | 各沉淀工具类；增 `--radius-xs`、`--overlay-bg` |
| P2 | settings.css:97 | range 滑杆固定 140px，窄设置窗挤压 label | 改 flex 自适应 |

## 二、主面板页（App.vue / FloatingPanel.vue / WindowApp.vue）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P1 | RecordList.vue:6 + PreviewPane.vue:504 | 列表列固定宽（min 280）+ 预览 min-width 280，窗口 <约 620px 且选中记录时溢出裁切；720px 断点未收预览列 | 窄宽度下预览改覆盖式/抽屉或禁用 |
| P1 | RecordList.vue:201-206, WindowApp.vue:31 | 分栏 resizer 仅指针拖拽，无 role=separator/tabindex/方向键（WCAG 2.1.1） | 加 separator 角色与键盘步进 |
| P1 | RecordList.vue:27-30 + useClipboardHotkeys.ts:94-105 | 方向键只改 selectedId 不移焦点，aria-activedescendant 挂在 tabindex=-1 容器上无效，读屏不同步 | 箭头键同时 .focus() 新选项 |
| P1 | RecordList.vue:760-783 | grid 卡 132/140px 固定高，字号/缩放放大时 meta 行被裁 | height 改 min-height |
| P1 | clipboard.ts:799-835 | 滚动中收到新记录 prepend，视口内容被「顶跳」68px 并伴随 row-flash | 非顶部时保持锚定或仅提示不插入 |
| P1 | RecordList.vue:149 | 行内收藏按钮失败时静默（快捷键/右键路径都有 toast），反馈不一致 | 统一失败提示 |
| P2 | RecordList.vue:356-360 | 切换 filter/排序重载期间保留旧列表且无加载指示，替换瞬间整列跳动 | 加列表级 loading/骨架屏 |
| P2 | RecordList.vue:303-315, 554-562, 369-374 | `sleep(150/160)` 等动画不受 anim-disabled 控制；scrollToTop 只看 prefers-reduced-motion 不看应用内开关 | JS 延时读取 enable_animation |
| P2 | useVirtualList.ts:109-118 | fillViewportIfNeeded 最多 3 轮，超高视口填不满时 footer 提示「滚动加载」但列表不可滚动，卡死 | 循环至填满或耗尽 |
| P2 | useClipboardHotkeys.ts:144 | Backspace 也可删除但右键菜单只提示 Del，隐藏死路 | 菜单/快捷键说明补齐 |
| P2 | FloatingPanel.vue:19-38 等 | 工具栏图标 13/14/15px 混用 | 约定三档并常量化 |

## 三、搜索栏（SearchBar.vue）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P2 | SearchBar.vue:187-195 | 自绘 14px/2px spinner 与全局 `.loading-spinner.small` 重复且不一致 | 复用全局类 |
| P2 | SearchBar.vue:22 | 搜索中 spinner 用 span+aria-label，无 role=status，读屏不播报 | 加 role=status |
| P2 | SearchBar.vue:197-208 等 | kbd 三套样式（SearchBar / settings.css / SettingsHelp） | 合并为单一 `.kbd` 工具类 |

## 四、预览面板（PreviewPane.vue）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P1 | PreviewPane.vue:1026-1034 | ≤720px 直接 display:none 第 4 个按钮（置顶），功能无替代入口 | 换行或收进溢出菜单 |
| P2 | PreviewPane.vue:108-117 | 图片点击外部打开，img 不可聚焦、无键盘等价 | 加 tabindex 与 Enter 处理 |
| P2 | PreviewPane.vue:135-139 | tag-remove 图标按钮仅 title 无 aria-label | 补 aria-label |

## 五、对话框体系（BaseDialog / ConfirmDialog / TagDialog / AliasDialog / WelcomeDialog）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P1 | BaseDialog.vue:88-94 | window 级 Escape 捕获未 stopPropagation，焦点不在卡片时一次 Esc 同时关弹窗+清选择/关面板，双重副作用 | 捕获层统一 stopPropagation |
| P1 | TagDialog.vue:56-73 | assign 模式 checkbox 被 hidden、label 不可聚焦，键盘完全无法分配标签 | 可见 checkbox 或 role=checkbox+tabindex |
| P2 | App.vue:207-223 | 失焦自动隐藏面板时若 ConfirmDialog 开着，弹窗随面板消失但 promise 悬挂，后续 confirm 被顶掉 | 隐藏前 settle 或挂起到窗口级 |
| P2 | ContextMenu.vue:28-31 | 主题 toggle 项用 menuitem 而非 menuitemradio，无 aria-checked；菜单关闭不还焦触发元素 | 改 menuitemradio + 焦点归还 |
| P2 | TrayMenuApp.vue:129-151 | 托盘菜单方向键只改视觉类，真实焦点留在 shell div | 加 aria-activedescendant 或真移焦 |

## 六、设置窗口（SettingsWindow.vue + 11 个子页）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P1 | settings.ts:70-91 | 设置保存失败仅 console.error 后静默回滚，用户无感知 | 至少一次性 error toast |
| P1 | SettingsAppearance.vue:104 | 字号范围 11–18px（最大 1.125×），达不到 WCAG 1.4.4 的 200% 缩放 | 上限提至 22px 或说明例外 |
| P2 | SettingsAppearance.vue:193-196 | 主题预览块硬编码 token 色值副本，token 变更会漂移 | 改用 var() |
| P2 | useBatchActions.ts:74 | 批量删除无确认，与单条永久删除的谨慎度不一致 | 加确认或统一策略 |
| P2 | SourceBadge.vue:70 | font-size 9px 低于刻度最小档（--text-xs=10px） | 升 10px 或扩展刻度 |
| P2 | 多处 | 展示级字号（1.125~2rem）绕过刻度；标题字重 600/700 混用 | 增 --text-2xl/3xl；标题统一 600 |

## 七、反馈与数据层（useToast / useBatchActions / store）

| 级别 | 位置 | 问题 | 建议 |
|---|---|---|---|
| P0 | clipboard.ts:464-478 等 | 假成功 toast（见 P0-1） | store 失败 rethrow，UI 决定提示 |
| P1 | useClipboardHotkeys.ts:39-63, useBatchActions.ts:17-62 | toast/confirm 文案硬编码中文，绕过 i18n | 改用 i18n key |
| P1 | useBatchActions.ts:61 | navigator.clipboard.writeText 未 try/catch，权限失败时 unhandled rejection | 包 catch + 错误 toast |
| P2 | useToast.ts:21-27 | 无相同文案去重，连按操作堆叠多条相同 toast | 同文案合并/替换 |
| P2 | clipboard.ts:431 | pasteRecord 就地 `copy_count += 1`，违反自身 patchRecord 约定，预览计数可能不刷新 | 走 patchRecord |
| P2 | ToastHost.vue:12 | toast 可点击关闭但不可聚焦、无关闭按钮 | 加可聚焦关闭钮 |
| P2 | WindowControls.vue:3,8,28 | 三个窗口按钮仅 title 无 aria-label | 补 aria-label |
| P2 | tauri.conf.json:17-19 | 主窗 resizable 但无 minWidth/minHeight，可缩到失控 | 设最小尺寸 |

---

## 修复优先级建议

1. **第一梯队（正确性/可读性）**：P0-1 错误反馈链路、P0-2 亮主题 type 色
2. **第二梯队（系统性对比度 + 高频交互）**：暗主题 accent 文字色、按钮体系统一、Esc 双触发、TagDialog 键盘可达、窄窗口预览溢出、新记录顶跳滚动
3. **第三梯队（打磨）**：图标/菜单/kbd/spinner 工具类沉淀、动画开关覆盖 JS 延时、aria-label 补齐、minWidth/minHeight

**做得好的地方**（保持）：token 覆盖率高（硬编码颜色仅约 15 处且多为合理场景）、虚拟列表+防抖+详情缓存的性能优化、危险操作确认覆盖全面、空态设计完整、reduced-motion 全局覆盖、对话框焦点陷阱实现规范。
