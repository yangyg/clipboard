# 移除「跟随系统」主题

## Status

Accepted

## Date

2026-08-06

## Context（背景）

设置页「外观 > 主题」原先提供「跟随系统」选项（`theme: "system"`），由 ADR-0002 的原生 watcher 驱动：Rust 不可见窗口监听 `WM_SETTINGCHANGE`（`ImmersiveColorSet`）+ 注册表 `AppsUseLightTheme`，emit `system-theme-changed`；前端用 `lastKnownSystemDark` 缓存 + matchMedia 兜底。

引入六个彩色预设主题（ADR-0003）后，两个问题浮现：

1. **语义冲突**：`system` 只解析到 dark/light，永远不会是彩色主题——「跟随系统」与「彩色主题是固定预设」的定位错位；用户选了 system 就看不到任何彩色。
2. **维护成本 vs 有限价值**：整条基建约 208 行 Rust（unsafe FFI）+ ~75 行前端 + ~15 个测试用例，只服务一个功能。而默认主题就是 `dark`，想用浅色直接选「浅色」或浅色彩色主题即可，自动化价值有限。

`matchMedia` 降级方案已被 ADR-0002 明确否决（WebView2 在隐藏窗口时不可靠触发 change 事件，面板大部分时间隐藏），因此保留 = 全量基建，无法简化。

## Decision（决策）

**完整移除「跟随系统」主题**：

- 删除 `src-tauri/src/system_theme.rs`（208 行）及 `lib.rs` / `setup.rs` 的引用。
- 前端移除：`settings.ts` 的系统主题追踪（matchMedia 监听、`lastKnownSystemDark` 缓存、`system-theme-changed` 事件守卫）、`useTrayTheme.ts` 的 system 分支与 `onSystemThemeChange`、`TrayMenuApp.vue` 的 `system-theme-changed` 监听。
- `settings.theme` 联合类型去掉 `'system'`；`THEME_CLASSES` 去掉永不会再被应用的 `"dark-theme"`（`dark` 走 `:root` 默认，不加 class）。
- 设置页主题卡去掉「跟随系统」及预览渐变，剩 9 张平铺卡片。
- 删除 `settings.spec.ts` 整个 system-tracking 测试块（~15 用例）；彩色主题测试改写为切到基础主题的清理断言。
- 文档：ADR-0002 标记为被本 ADR 取代（原文件保留为历史记录）。

**遗留值处理**：已保存 `theme: "system"` 的老用户在 `loadSettings` 时由 `normalizeSettings` 归一化为 `"dark"`，避免设置 UI 出现「无卡片选中」的悬挂状态。

## Consequences（后果）

**收益**

- 移除约 280 行基建 + 15 个测试 + ADR-0002 的功能前提；设置 UI 不再有与「固定彩色预设」冲突的选项。
- `system` 归一化保证存量数据平滑降级，无迁移。

**约束与注意**

- `theme` 联合类型不再含 `"system"`；任何从 DB 读到的 `"system"` 都必须经过 `normalizeSettings`（唯一归一化入口：`loadSettings` / `saveSettings` 快照 / 保存失败回读）。
- `.dark-theme` class 已无任何应用路径；`dark` 主题始终依赖 `:root` 默认变量，不加 body class（与之前一致）。
- 「跟随系统」相关代码禁止以任何形式回归（matchMedia 直读方案在隐藏 WebView2 下不可靠）。

**后续（out of scope）**

- 若未来重新引入「自动匹配 OS 深浅」，需重新评估原生 watcher（ADR-0002 的教训仍适用）。
