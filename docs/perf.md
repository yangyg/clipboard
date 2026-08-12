# 性能基准与关键路径计时

本文件记录 ClipVault 的性能目标、运行基准的方法，以及优化前后的实测数据。

## 关键路径计时（perf tracing）

Rust 侧的关键路径计时通过 `tracing::debug!` 输出在 `perf` target 下，默认日志过滤级别为
`info`，因此未启用时开销可忽略。启用方式：

```powershell
$env:RUST_LOG = "perf=debug"
npm run tauri dev
```

已埋点路径：

- `db_init` — 数据库初始化（含一次性迁移）
- `capture_text` / `capture_image` — 剪贴板捕获到落库 + 前端事件（worker 线程）
- `get_records` / `search_records` / `get_stats` / `get_all_tags` — 查询耗时
- `panel_show` — Rust 侧面板显示路径（快捷键/托盘唤起）

前端侧在 DevTools console 输出 `[perf] ...` 的 debug 日志（`performance.mark/measure`）：

- `clipvault:panel-show` — showPanel → 窗口显示 + 聚焦
- `clipvault:boot-to-records` — 启动到首屏记录就绪（首次）
- `clipvault:records-ready` — 每次首页加载耗时
- `clipvault:search-roundtrip` — 搜索 IPC 往返

## 运行基准

基准是 release-only 的 ignored 测试：种子 50,000 条文本 + 5,000 条图片记录（混合类型/长度，
含标签与 FTS 索引），输出各热路径 p50/p95。

```powershell
cargo test --release --manifest-path src-tauri/Cargo.toml -- --ignored perf --nocapture
```

## 目标与实测

规模基线：50k 文本 + 5k 图片（设计上限 20 万条）。验收方式：基准 + 计时，p95 为目标区间，
最终以实测为准。

| 指标 | 目标（p95） | 基线（优化前） | 实测（优化后） |
| --- | --- | --- | --- |
| `get_records` 首页 | ≤ 15ms | p50 72.9 / p95 142.6ms | p50 0.2 / p95 0.3ms ✅ |
| 搜索 3 字（FTS，命中全部 5 万行） | ≤ 50ms | p50 234.6 / p95 857.6ms | p50 76.4 / p95 89.2ms（FTS 全量命中排序，已知取舍） |
| 搜索 3 字（FTS，真实稀疏 ~250 行） | ≤ 50ms | 未测 | p50 5.2 / p95 6.3ms ✅ |
| 搜索 2 字（instr 全表） | 记录现状 | p50 42.0 / p95 201.0ms | p50 61–311 / p95 368–608ms（受机器波动影响大） |
| 搜索 1 字 | ≤ 200ms | p50 34.6 / p95 135.9ms | p50 0.2 / p95 0.7ms ✅ |
| `get_stats` 冷 | ≤ 200ms | 38.8ms | 29.9–44.3ms ✅ |
| `get_all_tags` 冷 | ≤ 50ms | 64.0ms | 15.7–64.8ms（静机 ~16–20ms）✅ |
| `get_all_tags` 缓存命中 | ≤ 1ms | 11.3ms（无缓存） | 0.00ms ✅ |
| 文本插入（含 FTS）≤10KB | ≤ 20ms | p50 9.5 / p95 13.1ms | p50 8.0 / p95 9.0ms ✅ |
| 图片编码 + 落盘（1280×720） | ≤ 200ms | p50 25.0 / p95 31.2ms | p50 17.0 / p95 18.2ms ✅ |
| 面板唤起（已有数据） | ≤ 150ms（前端 mark） | 待实机验证 | 标记已埋：`clipvault:panel-show` |
| 空闲 CPU（10 分钟均值） | ≤ 0.5% | 待实机验证 | 待实机验证 |
| 空闲 RSS | 较基线降 ≥ 10% | 待实机验证 | 待实机验证（读连接页缓存 16MB→8MB 已落地） |

说明：

- 基准为 release 构建、临时库种子 50k 文本 + 5k 图片。多次运行绝对数值有 ±2–5x 波动（受
  Windows Defender / 后台负载影响），表中取代表性的多次运行区间；趋势与相对改进稳定。
- 「搜索 3 字命中全部 5 万行」为病态场景：FTS5 trigram 需要对全部命中行计算 rank 后才能
  LIMIT，排序成本 ~70ms 属引擎固有开销；真实查询（命中数百行）为 ~5ms。
- 读连接页缓存由 16MB 降至 8MB（写连接保持 16MB），预期空闲 RSS 下降 ~24MB，待实机验证。
- 面板唤起 / 空闲 CPU / RSS 三项需在真实应用里用前端标记与任务管理器实测，标记已埋好。

手动冒烟项：20 次连续复制无丢事件；快捷键连按 10 次无卡顿；列表滚动流畅；托盘/唤醒后唤起正常。
