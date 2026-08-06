# 新增彩色预设主题：追加式整套 token，而非可自定义 accent

## Status

Accepted

## Date

2026-08-05

## Context（背景）

外观设置只有深色（默认）/ 浅色 / OLED / 跟随系统四种，accent 固定 Fluent 蓝 `#0078d4`。产品希望提供更多个性化空间，问题是「彩色主题」应当怎么落地。

三个候选方向：

1. **只做 accent 换色**：保留现有基底，只换主色。看似最小，实则最伤——CSS 中约 20 处硬编码 `rgba(0,120,212,…)`（`--accent-soft` / `--accent-softer` / `--bg-selected` / `--shadow-glow` / `--border-focus` 等）无法用任意色直接表达，必须重构为 `color-mix()`；且任意色会打穿精心调校的 AA 对比度和 Fluent 品牌气质。
2. **整套预设主题**：每个主题 = 背景系 + accent 家族 + 类型色 + 语义色一组完整的 CSS 变量块，逐个精调。token 结构已支持「一个主题 = 一块变量」，增量成本低。
3. **预设 + 自定义 accent 组合**：理想终点，但一步到位风险大。

配套澄清：若让彩色主题也跟随系统明暗，每个主题需要亮/暗两套 token（6–8 组），QA 面翻倍；剪贴板工具的强场景是暗色。

## Decision（决策）

1. **追加不替换**：保留 `dark|light|oled|system` 四个既有主题及其存量数据，新增三个固定彩色预设——紫夜 `dracula` / 冰蓝 `nord` / 暖橙 `sunset`。
2. **单变体、暗色基调**：每个彩色主题是一套完整 ~30 个 token 的固定块，不做亮/暗双变体；`theme === "system"` 永远只解析到 `dark`/`light`，绝不解析到彩色。
3. **扩 `theme` 联合类型**：`Settings["theme"]` 扩为 `'dark' | 'light' | 'oled' | 'system' | 'dracula' | 'nord' | 'sunset'`。Rust 端本就是 `String` 无校验，DB 零迁移；前端 `applyTheme` 的 `theme !== "dark"` 分支天然适配新 key，仅需把两处硬编码的 class 清理列表收敛为 `THEME_CLASSES` 常量。
4. **全套 token**：每个主题手写背景/border/文字/accent 家族/`--type-*`/success/warning/pin/danger/sensitive/shadows 全量，延续 AA 对比度纪律；**不引入 `color-mix` 重构**——accent 的 rgba alpha 按各主题手写。
5. **UI 平铺**：设置页全部 10 张主题卡平铺在**同一个 radiogroup**（不分组）。分组会暗示「深浅 × 色相可组合」的轴（例如浅色系 × 深色彩色系），而实际每个主题是独立完整的预设；平铺 + 自适应网格（`auto-fit minmax(96px,1fr)`）既消除歧义又适配窄屏。显示名用描述性色名，内部 key 保持稳定（dracula/nord/sunset + `-light` 后缀）。
6. **定位**：彩色主题是固定「调味」选项，不是「跟随系统」的替代；不提供自定义 accent 入口（留给未来版本）。

### 追加：浅色变体（紫霞/冰白/暖阳）

首发仅暗色基调，随后补充三个浅色变体：`dracula-light`（紫霞）/ `nord-light`（冰白）/ `sunset-light`（暖阳），与对应暗色家族共享 hue 与 accent 色相，但 token 整组按浅底精调（文字/类型/语义色加深以保证 AA）。浅色变体同样是**固定主题**，`system` 永不解析到它们。

### 追加：主题切换仅留在设置页

快捷菜单曾有一个二态「深色/浅色」开关。随着主题扩为 10 个平铺预设，该开关不再合适：二态控件无法表达 10 选一的设置，且点击会把用户的彩色主题直接丢弃为纯「深色/浅色」。已将其从侧栏快捷菜单移除，主题统一在 设置 > 外观 管理（侧栏底部设置按钮默认即落在外观分区）。

### 追加：themes/ 拆分 + 手绘主题族（手绘 / 手绘·浅）

- **token 文件按族拆分**：主题 token 块从 `main.css` 迁到 `src/styles/themes/*.css`（`base` / `dracula` / `nord` / `sunset` / `handdrawn`，暗+亮同族一文件），`main.css` 顶部 `@import` 引入（`@import` 须在 `:root` 规则之前）。主题块是 body 类作用域变量，相对 `:root` 的 html 级默认值靠继承生效，与加载顺序无关，纯搬迁零行为变化。
- **手绘主题族的非 token 视觉**：手绘主题在 token 之外还带一段**共享视觉覆盖块**（不规则圆角 `--sketch-radius*`、贴纸硬阴影、卡片微旋转、波浪下划线、纸点纹理、虚线 focus 环、马克笔 `::selection`），以 `:is(body.handdrawn-theme, body.handdrawn-light-theme)` 前缀门控，避免污染其余主题。因此「新增一个主题」的 checklist 从「token 块 + 入口 + 卡片 + i18n」扩展为「token 块 + 可选视觉块 + `themeClass.ts` 的 `THEME_CLASSES` 入口 + 设置卡 + i18n」。`THEME_CLASSES` 与 `applyTheme` 收敛为单一 `src/utils/themeClass.ts`，设置页与托盘窗口共用。
- **外层 `.panel-surface` 圆角仍跟随 `--panel-radius`**（与 Rust `SetWindowRgn` HWND 剪裁对齐），手绘波动只作用于内层卡片/菜单/按钮/徽章等元素。

## Consequences（后果）

**收益**

- 存量用户 `theme: "dark"` 等值继续有效，schema 与迁移零改动。
- 彩色主题自动覆盖全部界面（悬浮面板/窗口/托盘菜单/设置页共用 body class + token）。
- 对比度与品牌品质由「预精选 + 精调」保证，不受任意色打穿。

**约束与注意**

- **新增固定主题只需**：`main.css` 加一组 `body.<key>-theme` 块、`types.ts` 联合类型加值、`THEME_CLASSES` 自动纳入（`settings.ts` 与 `useTrayTheme.ts` 中 class 清理源）、`SettingsAppearance.vue` 加 `THEMES_COLOR_*` 项 + 预览渐变、i18n 两个 key。Rust / DB 不动。
- **`system` 永不解析到彩色主题**；彩色主题下忽略 `system-theme-changed` 事件（`applyTheme` 守卫天然覆盖）。
- 类型色与语义色必须随主题精调以保证 AA；不能只改背景 + accent 就上线。浅色变体的文字/类型/语义色整体加深。
- 若未来引入「自定义 accent」，需先做 `color-mix()` 重构，属独立工程（本 ADR 明确 out of scope）。

**后续（out of scope）**

- 自定义 accent / 用户自建主题。
