# 自定义托盘右键菜单设计

日期：2026-07-23  
状态：已确认（待实现）

## 目标

用与 ClipVault 主题一致的自定义悬浮菜单，替换原生系统托盘右键菜单。内容范围仅覆盖现有动作，不做最近记录或统计摘要。

## 非目标

- 不在菜单中展示最近剪贴板或今日条数
- 不改变左键托盘行为（显示/隐藏主面板）
- 不改粘贴、捕获、设置等业务逻辑本身

## 方案

独立 Tauri 小窗 `tray-menu`（无边框、透明、置顶、不进任务栏），右键托盘时在指针旁弹出。主窗隐藏时仍可用。

不采用：复用主窗浮层（与悬浮失焦逻辑冲突）；纯原生菜单美化（无法对齐应用主题）。

## 窗口与触发

| 项 | 约定 |
|----|------|
| 窗口 label | `tray-menu` |
| 外观 | `decorations: false`，透明，`alwaysOnTop: true`，`skipTaskbar: true`，默认不可见 |
| 前端入口 | 独立 `tray-menu.html` + `TrayMenuApp.vue`（不挂载主 `App.vue`） |
| 原生菜单 | 托盘**不**挂载系统右键菜单 |
| 左键 | 保持现状：切换主面板显隐 |
| 右键 | 计算位置 → `show` / `set_position` `tray-menu` |
| 关闭 | 失焦、Esc、点选任一项后立即 `hide` |
| CloseRequested | `prevent_close` + `hide`，不退出进程 |

定位：以托盘事件给出的屏幕坐标为锚点，按所在显示器工作区夹紧，避免贴边被裁切。

## 菜单内容与外观

顺序与分组：

1. 打开面板（图标 `panel`）
2. 暂停捕获 / 恢复捕获（图标 `pause` / `play`，随状态切换）
3. —— 分隔线 ——
4. 设置（图标 `settings`）
5. —— 分隔线 ——
6. 退出（危险样式，图标可用 `close`）

视觉：气质贴近主悬浮面板，而不是扁平系统菜单。

| 属性 | 约定 |
|------|------|
| 宽度 | 约 220px |
| 圆角 | `var(--radius-lg)`（约 14px），明显大于普通 ContextMenu |
| 阴影 | `var(--shadow-lg)`，与主面板同级 |
| 背景 | 跟主面板：`--bg-surface` + `panel-opacity`；若设置开启模糊则用同等毛玻璃（`blur-enabled`） |
| 边框 | `var(--border-default)` / `--border-subtle` |
| 内边距 | 略宽松（约 8px） |
| 标题 | 无品牌标题条 |
| 图标/文案 | Lucide 图标 + 中文，无 emoji |

交互仍对齐 `ContextMenu`：分隔线、hover、键盘上下 / Enter / Esc。主题跟随 dark / light / OLED（含 `system`）。

## 数据流

```
托盘右键
  → Rust: 定位并显示 tray-menu
  → 菜单窗: 拉取 pause 状态 + 应用主题

菜单项点击
  → invoke 专用命令（或等价 emit）
  → Rust 复用现有行为后 hide tray-menu：
      · 打开面板 → show_main_panel
      · 暂停/恢复 → 翻转 capture_paused，emit capture-paused
      · 设置 → 显示主窗 + emit open-settings
      · 退出 → app.exit(0)

状态同步
  · capture-paused：主端与菜单窗双向一致（打开时读一次；变更时广播）
  · theme：菜单窗启动时读 settings；主端改主题时广播，菜单窗 applyTheme
```

主窗现有对 `open-settings`、`capture-paused` 的监听保持不变。

## 文件与改动面（实现指引）

| 区域 | 改动 |
|------|------|
| `src-tauri/tauri.conf.json` | 注册 `tray-menu` 窗口；Vite 多页若需则同步 |
| `src-tauri/src/tray.rs` | 去掉原生 Menu；右键弹出；菜单命令接线 |
| `src-tauri/capabilities` | 为 `tray-menu` 补必要 window/event 权限 |
| `vite.config` / `index` | 增加 `tray-menu.html` 入口 |
| `src/TrayMenuApp.vue`（或等价） | 菜单 UI；主题与 pause；invoke |
| `src/components/TrayMenu.vue` | 可删除或改为指向新实现的说明，避免双源 |

可复用 `ContextMenu` 的键盘与结构，但**样式按主面板气质单独定**（更大圆角、更强阴影、可选毛玻璃），不要做成与列表右键菜单完全同款的小菜单。

## 测试清单

- [ ] 右键托盘弹出自定义菜单；无原生系统菜单
- [ ] 失焦 / Esc / 点选后菜单隐藏
- [ ] 打开面板、设置、退出行为与改前一致
- [ ] 暂停 ↔ 恢复文案与图标随状态变化；主面板暂停状态同步
- [ ] 切换主题后菜单外观一致；开启模糊时毛玻璃与主面板接近
- [ ] 圆角/阴影明显大于普通列表右键菜单，观感贴近主面板
- [ ] 左键仍只切换主面板
- [ ] 多显示器 / 屏幕边缘弹出不被裁切
- [ ] 关闭菜单窗标题栏关闭（若可触达）不退出应用

## 决策记录

- 内容范围：仅现有四动作（用户选 A）
- 实现形态：自定义面板（用户选 B）→ 独立小窗（用户选方案 1）
- 外观：贴近主面板（更大圆角、强阴影、可选毛玻璃；用户选外观 A）
