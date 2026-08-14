# AI 富集的双开关（`features.ai` × `enable_ai`）

## Status

Accepted

## Date

2026-08-14

## Context（背景）

捕获路径上新增了可选的 **AI 富集**：新插入的文本 / 代码 / 链接记录，可异步调用 OpenAI 兼容接口，把一句摘要写入 `alias`，并归纳少量自动标签。约束很硬：

1. **绝不能挡捕获热路径** — 队列满则丢任务，与图片 worker 同模式。
2. **隐私默认关闭** — 敏感记录永不送模；API Key 落盘走 DPAPI；正文截断到 `ai_max_chars`。
3. **产品能力可整块关掉** — 与标签 / 批量 / 同步 / 统计一样，走 `settings.features` 能力开关：关则藏 UI、拒命令、捕获不入队，数据保留。

如果只用一个开关，会出现两种坏默认：

- 跟其它能力一样默认 **开**：升级用户会在未配置 Key、未知情的情况下开始把剪贴板正文发到外部模型。
- 跟隐私开关一样默认 **关**：能力入口（设置导航「AI」）也一起消失，用户找不到配置页。

## Decision（决策）

拆成 **两层独立开关**，捕获入队与 worker 运行必须 **两者都开**：

| 开关 | 位置 | 默认 | 职责 |
|---|---|---|---|
| `features.ai` | 设置 → 功能模块 | **true**（缺字段升级安全，与其它能力一致） | 产品能力：关则隐藏设置导航「AI」、`ai_test_connection` 走 `require_feature`、捕获不入队。 |
| `enable_ai` | 设置 → AI | **false**（升级 JSON 缺字段也保持关） | 运行时：用户明确打开后才真正调用模型。关则设置页仍可见（能力开着时），便于填 Key / 测连接。 |

捕获侧（`capture.rs`）入队条件：`features.ai && enable_ai && ai_eligible_type && !is_sensitive && (ai_summary_alias \|\| ai_auto_tag)`，且 `AiConfig::is_configured()`。worker 循环里再次读 live settings，任一开关关掉即跳过积压任务。

敏感记录、图片 / 文件、未达 `ai_min_chars` 的短文本永不送模。Key 只以 DPAPI 密文存 SQLite settings blob。

## Alternatives Considered

### 单一 `enable_ai`

- Pros：心智更简单。
- Cons：无法把「整块卸掉 AI」与「配好了再开」分开；能力关了用户连配置页都进不去。
- Rejected：升级默认与可发现性互相打架。

### 单一 `features.ai`，默认 false

- Pros：隐私默认安全。
- Cons：与其它能力「缺字段 = 开」的升级约定冲突；关能力会拆掉整个设置分区，无法预填模型。
- Rejected：破坏 `FeatureFlags` 的 serde 缺省契约。

### 捕获线程内同步调用模型

- Pros：无需队列。
- Cons：模型延迟会卡住 clipboard worker，违反「捕获不丢、不堵」约束。
- Rejected：与图片 worker 的 `try_send` + 满队列丢弃模式相反。

## Consequences（后果）

- 新装与升级默认 **不会** 把剪贴板发到外部；用户必须先打开设置 → AI 的运行时开关。
- `features.ai = false` 时设置导航不显示 AI，即使本地已有 Key。
- 前端 `SettingsAi.vue` 只绑 `enable_ai`；导航显隐绑 `features.ai`。两层都要测。
- 文档 / 代理必须同时提到这两层，不能把 `enable_ai` 当成唯一开关。
