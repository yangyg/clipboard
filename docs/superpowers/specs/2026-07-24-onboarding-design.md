# 首次启动轻量引导设计

日期：2026-07-24  
状态：已确认（待实现）

## 目标

首次安装启动时，用一页欢迎弹窗说明核心用法（快捷键唤起 → 选记录粘贴 → 托盘右键），降低冷启动困惑。

## 非目标

- 不分步向导、不做界面 spotlight  
- 不在引导里改快捷键 / 自启 / 模式  
- 不提供「重新显示引导」入口  
- 升级老用户不弹引导

## 形态（用户选 A + 内容 B + 方案 1）

轻量单页欢迎弹窗，复用 `BaseDialog`。

### 文案与结构

| 元素 | 内容 |
|------|------|
| 标题 | 欢迎使用 ClipVault |
| 步骤 1 | 用全局快捷键唤起面板（展示当前 `global_shortcut`，如 `Ctrl+Shift+V`） |
| 步骤 2 | 选一条记录，回车或点粘贴 |
| 步骤 3 | 托盘图标右键：打开面板 / 设置 / 退出 |
| 主按钮 | 开始使用 |

- 遮罩不可点击关闭（`closeOnOverlay: false`），避免误关  
- Esc 等同「开始使用」（标记完成并关闭）  
- 无插画；样式跟现有 dialog token

## 数据与触发

### 设置字段

`onboarding_completed: boolean`

| 场景 | 值 | 行为 |
|------|-----|------|
| 全新安装 `Settings::default()` | `false` | 显示引导 |
| 升级：旧 JSON 无此字段 | 反序列化默认 `true` | 不显示 |
| 用户点「开始使用」 | 写 `true` 并 `save_settings` | 关闭，之后不再显示 |

Rust：

```rust
fn default_onboarding_completed() -> bool { true } // missing field = existing user

#[serde(default = "default_onboarding_completed", rename = "onboarding_completed")]
pub onboarding_completed: bool,

// in Default::default():
onboarding_completed: false, // brand-new install
```

前端 `types.ts` / `DEFAULT_SETTINGS`：`onboarding_completed: false`；`loadSettings` 合并时若服务端返回缺省已由 Rust 填好。

### 触发时机

`App.vue`：`await settingsStore.loadSettings()` 之后，若 `!settings.onboarding_completed` 则打开欢迎弹窗（面板已可显示，弹窗叠在上层）。

## 文件改动面

| 文件 | 改动 |
|------|------|
| `src-tauri/src/lib.rs` | Settings 字段 + default 函数 + Default |
| `src/types.ts` | Settings 类型 |
| `src/stores/settings.ts` | DEFAULT_SETTINGS |
| `src/components/WelcomeDialog.vue`（新建） | 欢迎 UI |
| `src/App.vue` | 加载后展示；完成回调写设置 |

## 测试要点

- [ ] 新库（或清掉 settings）首次启动弹出引导  
- [ ] 点「开始使用」后关闭，重启不再弹  
- [ ] Esc 完成并关闭，重启不再弹  
- [ ] 遮罩点击不关闭  
- [ ] 弹窗内展示的快捷键与设置中一致  
- [ ] 已有用户升级（JSON 无该字段）不弹  

## 决策记录

- 形态：轻量欢迎弹窗（A）  
- 内容：三步用法，不含改设置（B）  
- 持久化：`app_settings.onboarding_completed`（方案 1）
