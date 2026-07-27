# “跟随系统”主题：原生监听 WM_SETTINGCHANGE 而非依赖 matchMedia

## Status

Accepted

## Date

2026-07-27

## Context（背景）

设置页「外观 > 主题」提供「跟随系统」选项，期望应用随 Windows 深色/浅色模式实时切换。

最初的实现是纯前端方案：`theme === "system"` 时读取 `matchMedia("(prefers-color-scheme: dark)")` 并注册 `change` 监听器。该方案在 Windows 11 真机上**不生效**，根因：

- **WebView2 在宿主窗口隐藏时不可靠地触发 `matchMedia` change 事件**。ClipVault 的悬浮面板主窗口绝大多数时间处于隐藏状态（`visible: false`、失焦自动隐藏），恰好错过了系统主题切换的通知时机；窗口再次显示时也不会补发事件。
- 托盘菜单窗口同样是常驻隐藏的独立 webview，问题相同。

另一个必须澄清的边界：Windows 11「夜间模式」（设置 > 系统 > 显示，防蓝光色温滤镜）**不影响** `prefers-color-scheme`，任何应用都无法检测它，不属于「跟随系统」的目标信号；真正的信号是「深色模式」（设置 > 个性化 > 颜色）。

## Decision（决策）

### 1. Rust 原生监听，事件驱动前端

新增 `src-tauri/src/system_theme.rs`：独立线程创建**不可见顶层窗口**，接收 Windows 切换应用深浅色时广播的 `WM_SETTINGCHANGE`（lParam 为 `"ImmersiveColorSet"`），读注册表权威值后 `emit("system-theme-changed", dark: bool)` 到所有 webview。

**为什么不用 message-only 窗口（`HWND_MESSAGE`）**：message-only 窗口收不到广播型 `WM_SETTINGCHANGE`，必须是无父窗口的顶层窗口（不加 `WS_VISIBLE`，永不显示）。

**为什么注册表是权威值**：`HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`（DWORD，0 = 深色，1 = 浅色）在「自定义」模式下依然正确，且广播消息本身不携带新值。用 `AtomicI8` 对结果去重，避免重复广播导致重复 emit。

### 2. 前端缓存优先，matchMedia 降级为兜底

`settingsStore` 与 `TrayMenuApp.vue` 各自维护 `lastKnownSystemDark: boolean | null` 缓存：

- 原生事件与 matchMedia change 事件都刷新缓存，且**在固定主题下也持续刷新**——保证之后切回「跟随系统」时从最新 OS 状态起步。
- 任何重新应用 system 主题的时机（`loadSettings`、托盘菜单打开）**必须优先用缓存**（`lastKnownSystemDark ?? matchMedia(...).matches`），不得用新的 matchMedia 读取覆盖原生事件的正确结果——隐藏 webview 中 matchMedia 可能是过期值。
- `null`（尚无信号）才回退 matchMedia 当前值；webview 新建时 WebView2 的 `PreferredColorScheme: Auto` 会给出正确初始值，首个原生事件到达前不会出错。

### 3. 事件应用守卫

前端仅在 `theme === "system"` 时应用事件载荷；深色 / 浅色 / OLED 等固定主题不受系统切换影响（原有行为不变）。

## Consequences（后果）

**收益**

- 窗口隐藏期间系统切换主题，事件照常送达（JS 仍在运行），面板再次打开时主题已正确；主窗口与托盘菜单同时收到同一事件。
- 事件路径与 matchMedia 兜底互不冲突：两者都写入同一缓存，信号一致。

**约束与注意**

- **缓存优先是硬规则**：后续任何代码重新应用 system 主题时，禁止直接读 matchMedia 覆盖缓存（回归测试已覆盖：运行时 `loadSettings` 不得回退到过期 matchMedia 值）。
- watcher 线程随进程存活；应用 `RunEvent::Exit` 时 `PostMessageW(WM_CLOSE)` 优雅退出消息循环。`WATCHER_STARTED` 原子守卫防止重复启动。
- 非 Windows 平台为空实现（no-op），完全依赖 matchMedia 兜底路径。
- 「跟随系统」仅响应深浅色模式切换；Windows「夜间模式」（防蓝光）无法检测，产品上不做承诺。

**后续（out of scope）**

- macOS / Linux 若引入，需各自的原生信号源（如 `NSAppearance` 观察、D-Bus `org.freedesktop.portal.Settings`）。
