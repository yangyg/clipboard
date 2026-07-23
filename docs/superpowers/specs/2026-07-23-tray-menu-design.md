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

视觉：复用现有 `ContextMenu` 的 token 与交互（背景、圆角、分隔线、hover、键盘上下/Enter/Esc）。宽度约 200px，轻阴影，无标题、无 emoji。主题跟随应用的 dark / light / OLED（含 `system` 解析结果）。

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

可抽公共样式或薄包装复用 `ContextMenu` 视觉，避免两套菜单长相漂移。

## 测试清单

- [ ] 右键托盘弹出自定义菜单；无原生系统菜单
- [ ] 失焦 / Esc / 点选后菜单隐藏
- [ ] 打开面板、设置、退出行为与改前一致
- [ ] 暂停 ↔ 恢复文案与图标随状态变化；主面板暂停状态同步
- [ ] 切换主题后菜单外观一致
- [ ] 左键仍只切换主面板
- [ ] 多显示器 / 屏幕边缘弹出不被裁切
- [ ] 关闭菜单窗标题栏关闭（若可触达）不退出应用

## 决策记录

- 内容范围：仅现有四动作（用户选 A）
- 实现形态：自定义面板（用户选 B）→ 独立小窗（用户选方案 1）
