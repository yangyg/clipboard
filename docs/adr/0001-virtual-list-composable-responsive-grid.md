# 虚拟列表引擎抽取与响应式网格列数

## Status

Accepted

## Date

2026-07-26

## Context（背景）

前端两个组件膨胀到难以维护：

- `SettingsWindow.vue` 约 2101 行，承载全部设置分区（外观 / 快捷键 / 隐私 / 统计 / 标签 / 数据等）。
- `RecordList.vue` 约 1907 行，混合了记录渲染、工具栏、空状态，以及一套复杂的**虚拟滚动引擎**（list/grid 双布局、行高估算、二分查找定位滚动窗口、grid 行分组）。

其中 `RecordList` 的网格视图硬编码为 2 列，窄窗口（悬浮模式约 400px）下卡片过宽、体验差，需要响应式列数。

**关键约束**：虚拟滚动的行分组算法（`buildGridRows`）必须与 CSS 网格的**实际列数严格一致**——否则行高累加偏移与真实 DOM 位置错位，导致滚动跳变、空白、定位失灵。

## Decision（决策）

### 1. 虚拟化逻辑抽取为 composable，而非拆分 record-item 子组件

将整套虚拟滚动引擎抽到 `src/composables/useVirtualList.ts`：

- **拥有**：行高估算、`flatItems` / grid 行分组、布局签名（`layoutSig`）、滚动窗口二分查找、`displayItems` / `virtualPadTop` / `virtualPadBottom`、viewport 填充、ResizeObserver。
- **宿主保留**：`RecordList.vue` 保留模板渲染、记录项交互（粘贴 / 收藏 / 置顶 / 删除）、右键菜单、布局偏好持久化。

**为什么不把 record-item 拆成子组件**：其样式与 list/grid 布局深度交织（大量 `.view-grid .record-item .xxx` 后代选择器），强行拆分需要大规模样式迁移，风险高、收益低。composable 抽取拿到了主要收益（逻辑内聚、可测试、宿主瘦身），同时避开样式重构风险。

工具栏、空状态这类**边界清晰**的部分仍拆为子组件：`ListToolbar.vue`、`ListEmptyState.vue`。

### 2. 响应式网格列数：JS 单一数据源，而非 CSS auto-fill

- `gridCols` 为 `ref`，由 ResizeObserver 监听容器内容宽度推导（`GRID_MIN_CARD_WIDTH = 200`）。
- 宿主组件用 inline style `grid-template-columns: repeat(${gridCols}, minmax(0, 1fr))` 应用列数。
- `watch(gridCols)` 触发 `buildGridRows` 重新分组。

**为什么不用 CSS `repeat(auto-fill, minmax(...))`**：那样列数只存在于 CSS 渲染结果中，JS 行分组无从得知真实列数，两个数据源必然漂移。用 JS 计算 + inline style 保证**列数只有一个来源**，分组算法与渲染永远一致。CSS 中保留 `repeat(2, ...)` 仅作兜底（会被 inline style 覆盖）。

### 3. SettingsWindow 按分区拆分

`settings/` 下 10 个分区子组件 + `useSettings` composable（共享 store 访问）+ 全局 `settings.css`（共享原语）。快捷键录制的窗口级监听逻辑留在父组件，通过 props/emit 与 `SettingsShortcuts` 通信。

## Consequences（后果）

**收益**

- `RecordList.vue` 1907 → 1174 行；`SettingsWindow.vue` 2101 → 336 行。
- 虚拟化引擎内聚、可独立演进与测试；网格在窄窗口降为 1 列、典型宽度保持 2 列。

**约束与注意**

- 网格列数**只能**通过 `gridCols` 修改；直接改 CSS 的 `grid-template-columns` 不会生效（inline style 覆盖）且会破坏虚拟化。
- 行高 / 卡片高的估算常量（`GRID_CARD_HEIGHT` 等）与 CSS 耦合，改样式需同步改 composable 常量。
- `useVirtualList` 内部使用 Pinia store，仅可在组件 setup 中调用。

**后续（out of scope）**

- 若确需拆分 record-item 子组件，应先重组样式（解除 `.view-grid .record-item` 后代选择器交织）再进行。
