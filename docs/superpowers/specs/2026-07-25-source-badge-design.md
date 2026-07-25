# Source Badge（来源首字母色块）设计

## Status

Accepted（待实现）

## Date

2026-07-25

## Goal

列表与详情里的来源（WorkBuddy / 系统剪贴板 / msedge 等）目前主要靠纯文字识别，扫视成本高。先用**稳定着色的首字母色块 + 短名**降低识别成本；Windows 真实应用图标后置。

## Scope

| 包含 | 不包含（本期） |
|------|----------------|
| 列表行 meta 来源 | 从 exe 抽取真实图标 |
| 详情顶部来源 meta | 改 DB / IPC / `source_app` 语义 |
| 共享工具函数 + `SourceBadge` 组件 | 网格布局大改 |
| 工具函数单测 | 设置项开关 |

## Decisions（已确认）

1. **范围**：列表 + 右侧详情都加。
2. **形态**：先做首字母色块；真实图标以后再加。
3. **文案**：色块 + 现有短名（悬停可用 `title` 看完整源）。
4. **实现**：共享 util + 小组件，不就地复制两份逻辑。

## Appearance

- 色块约 **14×14**，小圆角方块（非正圆），内嵌 **1 个字**（近白、字重约 600），右侧短名不变。
- **取字**（对短名）：
  - 若存在拉丁字母或数字 → 取第一个，并大写（`msedge`→`M`，`WorkBuddy`→`W`）。
  - 否则取第一个字符（中文应用名用首字）。
  - 空 `source_app` / 显示「系统剪贴板」→ 固定字 **`剪`**，中性灰底（不进彩色调色板）。
- **颜色**：对完整 `source_app` 字符串稳定哈希到现有 8 色调色板；同应用同色。空来源用 `--text-tertiary` 系灰。
- 色块 `aria-hidden="true"`；可读文本仍是短名；整段可设 `title`（完整源或短名）。
- 列表 / 网格 / 详情共用同一组件与尺寸，避免网格卡片被撑破。

## Data

- 仅使用现有字段 `ClipboardRecord.source_app`（可为空字符串）。
- 短名规则与现网一致：去路径、去 `.exe`；空 →「系统剪贴板」。

## Architecture

### `src/utils/sourceBadge.ts`

- `sourceShortName(sourceApp: string): string`
- `sourceInitial(shortName: string, sourceApp: string): string`
- `sourceAvatarColor(sourceApp: string): string`（空 → 灰；否则调色板哈希）
- `resolveSourceBadge(sourceApp: string): { label: string; initial: string; color: string }`

### `src/components/SourceBadge.vue`

- Props：`sourceApp: string`；可选 `title?: string`；预留可选 `iconSrc?: string`（本期不传；有则渲染 `<img>`，无则字母色块）。
- 默认渲染：色块 + 短名文本。
- **搜索高亮**：色块不参与 HTML 高亮。列表在搜索时对短名使用现有 `highlightSearchHtml`——组件提供：
  - 默认模式：内部渲染 escaped 短名；或
  - `labelHtml` prop（已消毒/高亮的 HTML 片段）仅替换文字节点，色块仍由组件渲染。
- 样式 scoped；色块尺寸固定，列表与详情共用。

### Call sites

- **RecordList**：移除 `.source-dot` 与 `sourceHtml` 内联圆点；meta 使用 `SourceBadge`（搜索时走 `labelHtml`）。删除仅服务于圆点的重复着色逻辑（迁到 util）。
- **PreviewPane**：来源一行改为 `SourceBadge`。

### Future (out of scope)

- 捕获或懒加载时用 Windows API 抽图标，缓存后通过 `iconSrc` 传入；失败继续字母色块。

## Testing

`sourceBadge.ts` 单测至少覆盖：

- 空字符串 → 标签「系统剪贴板」、字「剪」、灰色
- 带路径的 `.exe` → 短名与首字母正确
- 大小写拉丁名 → 大写首字母
- 中文短名 → 首字
- 同一 `source_app` 两次着色结果相同

## Non-goals / Constraints

- 不引入新依赖。
- 不因色块改变虚拟列表行高估算（14px 落在现有 meta 行内）。
- 不做「多彩主题包」或自定义来源色。
