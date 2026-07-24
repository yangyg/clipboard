# ClipVault 性能审查报告

**审查日期**: 2026-07-24  
**修订**: 2026-07-24（对照 `LIST_SOFT_CAP` / 现有缓存策略校正严重度；落地项见文末）  
**审查范围**: 全代码库（Rust 后端 + Vue/TypeScript 前端），以性能为核心维度  
**项目路径**: `D:\workspace\ClipVault`

> 原文部分条目将前端列表规模按 `max_records=1000` 估算。实际内存列表受 **`LIST_SOFT_CAP = PAGE_SIZE * 2 = 120`** 约束，因此若干「严重」项在真实运行时更接近中/低优先级微优化。

---

## 高优先级（值得做）

### H4 — `get_record_list` 在 capture worker 中产生冗余 DB 查询

**文件/位置**：`src-tauri/src/lib.rs`（`process_capture_job`）、`src-tauri/src/db/mod.rs`（`insert_record`）

**现象**：`insert_record` 只返回 `(id, is_new)`，随后再 `get_record_list` 做一次 SELECT + 读锁，仅为 emit `clipboard-changed`。

**影响**：每个 capture 额外一次 SQLite round-trip（约数十～数百 μs），连续复制时累加。相对其它条目，这是后端路径上更实在的收益。

**建议**：持写锁期间用 `RECORD_COLS_LIST` 读回 list 形记录并一并返回（去掉二次读锁）；`apply_auto_tags` 之后仅 `get_record_tag_names` 刷新 tags。

**状态**：已落地（见文末）。

---

### S1 — 虚拟滚动视野计算为线性扫描

**文件/位置**：`src/components/RecordList.vue`，`virtualRange`

**现象**：`flatItems` 按 `offset` 单调递增，但 `start`/`end` 用两个 `while` 线性推进。

**校正**：内存行数通常 ≤120（另加置顶 label），单帧线性扫描开销很小；报告原文「1000+ 项 / 可感知掉帧」偏高。仍适合改为二分，改动小、正确性清晰。

**建议**：对 `offset+height` / `offset` 做二分下界查找，再加 `OVERSCAN`。

**状态**：已落地（见文末）。

---

### H2 — `recordsById` 在记录变更时全量重建 Map

**文件/位置**：`src/components/RecordList.vue`

**现象**：`filteredRecords` 任意字段变化都会重建整表 `Map`，再驱动 `windowItems`。

**校正**：n≈120 时重建成本约亚毫秒级。不宜只挂在 `layoutSig` 上重建 Map 后直接当 live 数据源——收藏/粘贴次数等字段变更不会改 layout，列表会显示陈旧行数据。

**建议**：`layoutSig` 时只重建 `id → index`；`windowItems` 仍读取 `filteredRecords[index]`，既减少 Map 重建，又保持内容响应式。

**状态**：已落地（见文末）。

---

### S2 — `onNewRecord` 热路径多次遍历 / 多次赋值

**文件/位置**：`src/stores/clipboard.ts`，`onNewRecord` + `trimRecordsSoftCap`

**现象**：`findIndex` + `filter(pin)` + soft-cap 两次 `filter` + spread，多次触达响应式。

**校正**：同样受软上限约束；「持续 Ctrl+C 主线程冻结」对当前规模偏夸张。整理成单次扫描 + 一次赋值仍有利于清晰度与 GC。

**状态**：已落地（见文末）。

---

## 中优先级（可选）

### S3 — `insert_record` 溢出探针每次插入都执行

**文件/位置**：`src-tauri/src/db/mod.rs`

**现象**：每次 insert 后用 `LIMIT max+1` 探针是否超 cap；多数用户长期达不到 `max_records`。

**校正**：探针本身很轻（扫描行数有上界）。原子近似计数可减少查询，属锦上添花，原「严重」定级过高。

**建议**：`approximate_live_count` 仅在逼近阈值时跑 SQL；删/回收站路径同步递减。

**状态**：未做。

---

### H1 — 文本 capture 双重 SHA-256

**文件/位置**：`process_capture_job`：`sha256_hash(&captured.fingerprint())`

**现象**：`fingerprint()` 已是 plain+html 的 SHA-256 hex；再 hash 一次是为历史 DB 行兼容。

**影响**：μs 级。去掉需双 hash 查询或一次性迁移，性价比一般。

**状态**：保留现状。

---

### H3 — `is_ignored_app` 每次比较分配临时 String

**现象**：每次 capture 对 `source_app` 与各 pattern `to_lowercase()`。

**影响**：默认 pattern 很少，可忽略。设置加载时预规范化即可。

**状态**：未做。

---

### M1 — `windowItems` 对可见行调用 `recordThumbSrc`

路径拼接 + `convertFileSrc`；非图片返回 `null`。可缓存到 store，收益低。

### M2 — `scheduleLoadStats` 双 timer

800ms debounce + 5s max-wait；逻辑正确，可简化代码，非性能瓶颈。

### M3 — `scheduleExpireSweep` 全表扫 `auto_expire_at`

敏感过期行极少；可维护单独 expire 列表，属微优化。

### M4 — 搜索 SQL 动态拼接

搜索低频；`prepare_cached` 模板化属可维护性项。

---

## 低优先级 / 已确认

### L1 — `image_quick_fingerprint`

仅在序列号变化且存在 bitmap 时跑；4KB SHA-256 可忽略。保留。

### L2 — `media::cached_media_dir_size`

**已满足建议**：TTL **120s**，图片写入/删除增量调整缓存，不必每次 stats 全盘扫描。

### L3 — `schedule_persist_window_size`

已有 resize debounce；首次落盘读 settings 可接受。

---

## 建议落地顺序（修订）

| 顺序 | 编号 | 说明 | 风险 |
|------|------|------|------|
| 1 | H4 | 去掉 capture 后冗余 `get_record_list` | 中（返回类型 / auto-tag tags） |
| 2 | S1 | 虚拟滚动二分 | 低 |
| 3 | H2 | id→index + live records | 低 |
| 4 | S2 | `onNewRecord` 单次扫描 | 低 |
| 5 | S3 / H3 / M* | 按需 | 低～中 |
| — | H1 | 不建议优先（历史 hash） | 中 |

---

## 架构亮点（确认保留）

- **`sync_channel(2)` + `try_send`**：poll 与 capture worker 解耦，满队列丢弃不阻塞
- **`clipboard_sequence_number` 门控**：未变更则跳过重读取
- **`downscale_captured_rgba_if_large`（2560）**：进 channel 前限峰值
- **FTS5 trigram + 短查询 `instr()`**
- **`insert_record` 单写锁**：hash 去重无 TOCTOU
- **列表 `substr(content,1,400)` + `content_len`**：IPC 截断
- **`DETAIL_CACHE_MAX = 6`**、**`LIST_SOFT_CAP = 120`**
- **`flatItems`：`shallowRef` + `layoutSig`**：内容变更不重建虚拟布局几何

---

## 本轮落地

- 文档严重度与软上限表述校正（本文）
- **H4 / S1 / H2 / S2** 代码优化（同会话实现）
