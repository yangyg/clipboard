# 剪贴板监视器事件驱动（AddClipboardFormatListener）

## Status

Accepted

## Date

2026-08-11

## Context（背景）

原实现是独立线程上的固定 **250ms 轮询**：每 tick 调用 `GetForegroundWindow`（前台追踪）+ `GetClipboardSequenceNumber`，序列号未变则跳过所有读取；变化时才真正读文本/位图，且只有全部读取成功才推进序号水印；`ClipboardOccupied` 失败时不推进水印，靠下一个 tick 重试；paste 后 1.5s 抑制窗口跳过读取且不推进水印，窗口结束后重读一次并由 hash 去重吸收。

轮询的问题：

1. **空闲功耗**：即使没有任何复制，仍以 4 次/秒持续唤醒（每次至少两次 user32 调用）。
2. **延迟**：从复制发生到入库存在 0–250ms 的固定延迟。

事件驱动的候选方案（`AddClipboardFormatListener` + message-only 窗口收 `WM_CLIPBOARDUPDATE`）可同时解决这两点，但自身缺少轮询「免费提供」的三项保障：占用后的重试节奏、睡眠唤醒后的补采、注册失败时的兜底。另外，一次「逻辑复制」常常触发**多条** `WM_CLIPBOARDUPDATE`（应用分次设置文本 / HTML / 位图格式），若逐事件读取，会对同一复制反复执行昂贵的 `get_image()` 全量 RGBA 拷贝。

## Decision（决策）

Windows 采用**事件驱动为主 + watchdog 兜底**的混合方案：

- 消息线程创建 message-only 窗口（`HWND_MESSAGE`），`RegisterClassW` + `CreateWindowExW`，`AddClipboardFormatListener(hwnd)` 注册，`GetMessageW` 循环处理消息。
- `WM_CLIPBOARDUPDATE` → 重置 **150ms debounce timer**（同一 timer id 的 `SetTimer` 天然重置窗口，把同一次复制产生的多条通知折叠成一次读取），到期后执行一次 `handle_clipboard_tick`。
- `TIMER_WATCHDOG`：**1s 周期**执行 `handle_clipboard_tick` + 前台追踪。职责：
  - 睡眠/唤醒补采（睡眠期间剪贴板被修改，唤醒后序号检查发现差异即补采）；
  - `ClipboardOccupied` 后 **250ms 快速重试**（busy 时把 watchdog 周期缩到 250ms）；
  - `AddClipboardFormatListener` 注册失败时降级为 1s 轮询（延迟变大但捕获不丢）。
- 所有 arboard 剪贴板访问（基线指纹、句柄、读取）集中在该消息线程，避免跨线程 `OpenClipboard` 竞争。
- 非 Windows 保留原 250ms 轮询路径（本应用 Windows-only，仅为保持跨平台编译）。
- 共享 `handle_clipboard_tick`：文本指纹 / 图片快速指纹 / 抑制窗口 / 序号水印语义与旧轮询完全一致；水印只在全部读取成功后推进。
- 前台追踪降频：`track_last_foreign_foreground` 从每 tick 改为搭 watchdog 的 1s 节奏；paste 前的 `resolve_paste_target` 仍显式刷新，目标准确性不受影响。

## Alternatives Considered

### 纯事件驱动（无 watchdog）

- Pros：空闲零唤醒，实现更简单。
- Cons：`ClipboardOccupied` 没有自然重试节奏；睡眠唤醒漏采；监听注册失败会静默丢捕获。
- Rejected：可靠性退化，违反「捕获不丢」的核心约束。

### SetWinEventHook(EVENT_SYSTEM_FOREGROUND) 前台追踪事件化

- Pros：前台追踪也达到零唤醒。
- Cons：额外 hook 复杂度；1s 周期追踪成本已极低（一次 `GetForegroundWindow` + 原子比较），且 paste 关键路径有显式刷新。
- Rejected：收益不足以覆盖复杂度。

## Consequences（后果）

**收益**

- 空闲唤醒从 4 次/秒降为 0；捕获延迟从 0–250ms 降为约 150ms（通知亚毫秒 + 事件合并窗口）。
- 同一次复制的重复 `get_image()` 被合并窗口消除；快速连续复制时丢失窗口反而更小（150ms < 250ms）。
- 注册失败 / 占用 / 唤醒场景均有 watchdog 兜底，可靠性不低于原轮询。

**约束与注意**

- 剪贴板访问必须留在消息线程（arboard 线程亲和），不允许从其他线程直接调用。
- debounce 150ms 意味着「复制后 150ms 内被覆盖」的中间值不保留——与轮询的 250ms 窗口相比更短。
- `stop()` 依赖 `WM_TIMER` 唤醒，退出延迟 ≤1s（轮询时为 ≤250ms）。
- 前台追踪最坏延迟 1s，但 paste 前显式刷新保证目标准确性。

**后续（out of scope）**

- 若未来需要前台追踪也零唤醒，可评估 `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`（见 Alternatives）。
