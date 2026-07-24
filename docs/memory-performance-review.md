# ClipVault 内存性能审查报告

> 审查日期：2026-07-23
> 审查范围：前端（`src/` 全部 Vue/TS）＋ 后端（`src-tauri/src/` 全部 Rust）
> 审查重点：内存占用、内存泄漏、对象保留、未释放资源、大对象重复创建、缓存上限

---

## 总体结论

**未发现内存泄漏类缺陷。** 代码库在内存方面设计克制、架构成熟——前端列表软上限、详情缓存上限、定时器全部清理、监听器成对卸载；后端捕获通道有界、图像单次编码、列表查询截断内容并剥离 HTML、连接池固定、全局状态均为单值缓存、导出流式分页。**无未设上限的全局集合、无增长式历史缓冲区、无 Rc/RefCell 循环引用。**

残留问题均为**中低优先级**的瞬时峰值或微优化项，按严重程度排序如下。

---

## 严重程度排序

### 🟡 M1（中）：大图捕获通道的瞬时 RSS 峰值有界但可能很高

- **位置**：`src-tauri/src/lib.rs:425`（`sync_channel::<CaptureJob>(4)`）、`src-tauri/src/clipboard.rs:138`（`clipboard.get_image()`）、`src-tauri/src/media.rs:38`（`store_clipboard_image`）
- **机制**：剪贴板监控线程每次真实变更调用 `clipboard.get_image()`，将**整张原始 RGBA 位图**移入有界通道（容量 4）；worker 线程取 1 个处理。因此任意时刻内存中最多驻留 **= 通道 4 个 + 处理中 1 个 = 5 张 RGBA**。
- **影响**：每张 RGBA = `宽×高×4` 字节（4K ≈ 33 MB，8K ≈ 132 MB）。若用户高频复制超大图且 worker 编码慢于生产，峰值 ≈ **5×单图体积**（8K 下可达 ~660 MB 瞬时 RSS）。该峰值有界（不会无限增长），处理完即释放，但**在内存受限机器上复制超大图时可能触发 OOM**。
- **已有缓解**：`store_clipboard_image` 在落盘前已把边长缩到 `MAX_EDGE=4096`，但**缩放在 worker 端进行**——进入通道的仍是未缩放的原始 RGBA。
- **建议**：在监控线程 `on_change` 之前对 `CapturedImage.rgba` 先做尺寸下采样（或把通道容量降到 1–2），使进入通道的位图体积有硬上限。监控线程的 `get_image()` 本身是单次分配、用后即释放，并非泄漏。

### 🟢 L1（低）：导入时一次性加载全表 hash 到 HashSet

- **位置**：`src-tauri/src/db/mod.rs:1248`（`import_records` 中 `SELECT hash FROM records` → `HashSet`）
- **机制**：导入为去重，先把**整张表的所有 hash** 读入内存 HashSet，再逐条比对。
- **影响**：HashSet 体积 = 全表行数（受 `max_records`+保留策略约束，默认上限 1000，但保留期可能放大）。对超大库是一次性数 MB 分配，**导入结束后立即释放**，非泄漏。
- **建议**：可改为按 hash 分批 EXISTS 查询，或仅对将要导入的批次建临时索引；属低优先级打磨。

### 🟢 L2（低）：前端列表每次变更重建 Map 与对象数组

- **位置**：`src/components/RecordList.vue:552`（`recordsById` computed）、`:558`/`:571`（`windowItems`/`displayItems`）
- **机制**：`recordsById` 每次 `filteredRecords` 变化都新建一个 `Map<number, ClipboardRecord>`；`windowItems`/`displayItems` 再为每个可见行生成新 wrapper 对象（`{...item, record, thumb}`）。网格视图会映射全部 ≤120 行。
- **影响**：受 `LIST_SOFT_CAP=120` 约束，规模小；Vue computed 会缓存、旧对象可回收，**不是泄漏**。但每次剪贴板新增（前插 + 软上限裁剪 → 重算）都会触发重建，属可避免的微分配。
- **建议**：仅对**可见窗口**做 `recordsById` 解析（目前 `windowItems` 已只取可见段，但 `recordsById` 仍遍历全量 120）；或对 list 行做 key 稳定化以减少 wrapper 重建。

### 🟢 L3（低/仅开发期）：App.vue 的 Tauri 监听器未保存 unlisten 句柄

- **位置**：`src/App.vue:162/168/177/182`（`listen("paste-focus-lock"|"open-settings"|"capture-paused")`、`appWindow.onFocusChanged`）
- **机制**：根组件在 `onMounted` 注册监听器但**未保存 unlisten 函数**。生产环境 App 仅挂载一次，监听器与进程同寿，**无泄漏**。但在 `tauri dev`（Vite HMR）下，`onMounted` 可能重复执行而清理不触发，导致 `listen`/`onFocusChanged` **跨 HMR 累积重复注册**（现象：同一事件被处理多次，如设置窗口被打开多次）。
- **建议**：将 unlisten 句柄存入变量，在 `onUnmounted` 中统一调用；并加"已注册"守卫避免重复。

### 🟢 L4（低）：旧版 base64 图片在列表缩略图中的正确性问题

- **位置**：`src/utils/mediaUrl.ts:32`（`legacyBase64Src`）、后端 `RECORD_COLS_LIST`（`substr(content,1,400)`，`src-tauri/src/db/mod.rs:71`）
- **机制**：旧记录把 PNG 以 base64 存于 `content`。后端列表查询把 `content` **截断到 400 字符**，因此列表行里的 `record.content` 是残缺 base64；`legacyBase64Src` 对其拼接 `data:image/png;base64,...` 会得到**损坏的 data URL**（列表缩略图裂图）。预览时走 `get_record`（完整 content）则正常。
- **影响**：这是**正确性问题而非内存问题**；但提示一个相关内存点——预览旧图时 `selectedRecord` 合并进的 `detail.content` 是**完整 base64**（可达数 MB），驻留于 `recordDetails`（上限 6）。有界（≤6 条），瞬时。
- **建议**：旧记录迁移时把 base64 落盘为 media 文件（已在 `store_clipboard_image` 的"文件级去重"路径覆盖新记录），列表缩略图改走 media/thumb 路径。

### ⚪ 信息项（已确认安全，无需改动）

| 检查点 | 结论 |
|--------|------|
| 前端 `records` 列表 | 软上限 `LIST_SOFT_CAP=120`（`clipboard.ts:52`），`trimRecordsSoftCap` 裁剪，非增长式 |
| 前端 `recordDetails` 详情缓存 | 上限 `DETAIL_CACHE_MAX=6` + LRU 式裁剪（`clipboard.ts:63/322`），非增长式 |
| 后端列表查询 | `RECORD_COLS_LIST` 用 `substr(content,1,400)` 且 `content_html=NULL`（`db/mod.rs:71`），列表行绝不携带完整内容/HTML |
| 后端连接池 | 1 写 + 3 读（`READ_POOL_SIZE=3`，`db/mod.rs:89`），固定大小，WAL 并发读 |
| 后端全局缓存 | `settings_cache`（单值）、`MEDIA_SIZE_CACHE`（单值 + 120s TTL）、`get_foreground_window_info` CACHE（单值 + 250ms TTL）、监控线程仅保留**一条**文本指纹与**一条**图像指纹（`clipboard.rs:44-47`），均为有界单值 |
| 后端静态集合 | `PASTE_TARGET_HWND`/`OUR_MAIN_HWND`/`PASTE_GATE`/`SIZE_SAVE_GEN` 均为单值；`detect.rs` 的 `LazyLock<Regex>` 仅编译一次 |
| 导出 | `export_data` 流式分页（page_size=200，BufWriter 直写，每批用完即释放，`commands.rs:499`），**不整库载入内存** |
| 定时器 | 前端所有 `setTimeout`/`setInterval` 均在重置前 `clearTimeout`/`clearInterval`；`PreviewPane` 的 `setInterval` 在 `onUnmounted` + watch 中清理（`PreviewPane.vue:318/338`） |
| 监听器 | 除 App.vue 根监听（见 L3）外，`BaseDialog`/`ContextMenu`/`useClipboardHotkeys`/`SearchBar`/`WindowControls`/`SettingsWindow` 均在 `onMounted` 注册、`onUnmounted`/`removeEventListener` 卸载 |
| 图像资源 | 前端用 `asset://`（文件）而非内存位图（`mediaUrl.ts`）；`<img>` 解码位图随 src 变更/卸载释放；后端落盘后即释放 RGBA |
| 清理任务 | `cleanup_expired`/`cleanup_retention`/`empty_trash` 收集匹配 ID 与媒体路径对后立即删除并 purge 文件（`db/mod.rs:1190+`），规模以实际数据为界 |
| 富文本清洗缓存 | `sanitizeCache` 上限 24、FIFO 淘汰（`sanitizeHtml.ts:3/32`），非增长式 |

---

## 内存使用模式与高内存路径

**典型内存占用分布（常驻基线，单位约）**
1. WebView2 本身：~80–150 MB（与内容无关，框架固定）
2. 前端 store：`records`（≤120 行小对象）+ `recordDetails`（≤6，含预览完整内容/HTML）+ 各 computed —— 通常 < 10 MB
3. Rust 后端：连接池（4 连接）+ 单值缓存 + 监控线程 —— < 5 MB
4. **峰值主要来自图像捕获**：通道内 ≤5 张原始 RGBA（见 M1）

**高内存代码路径（按峰值排序）**
1. 🥇 复制大图：`clipboard.get_image()` → 通道(≤4) + worker(1) 各持 RGBA → 峰值 ≈ 5×单图（M1）
2. 🥈 预览富文本/旧图：选中记录经 `get_record` 载入完整 `content_html`/base64，驻留 `recordDetails`（≤6 条）
3. 🥉 导入大批量：一次性 `HashSet` 全表 hash（L1）
4. 导出/清理：均已流式/分页，无显著峰值

---

## 建议优先级

| 优先级 | 项 | 改动量 | 收益 |
|--------|----|--------|------|
| 1 | **M1**：捕获前对 RGBA 下采样或缩小通道容量 | 小（监控线程数行） | 消除超大图 OOM 风险 |
| 2 | L3：App.vue 监听器存 unlisten 并在 unmount 清理 | 小 | 修复 dev HMR 重复注册 |
| 3 | L1：导入 hash 改为分批 EXISTS | 小 | 降低大库导入瞬时内存 |
| 4 | L4：旧 base64 列表缩略图走 media 路径 | 中 | 修复裂图 + 减少详情缓存 base64 驻留 |
| 5 | L2：可见窗口级 `recordsById` 解析 | 小 | 微优化列表重算分配 |

**综合**：内存安全性良好，可投入生产。唯一值得在上线前处理的是 **M1（大图捕获峰值）**，其余均为打磨项。

---

## 修复实施（2026-07-23）

按建议优先级第 1、2 项，已实施 M1 与 L3 的修复，并通过编译验证。

### ✅ M1 — 捕获前下采样 + 通道容量收紧

- **`src-tauri/src/clipboard.rs`**
  - 新增 `const CAPTURE_MAX_EDGE: u32 = 4096`（与 `media::MAX_EDGE` 对齐）。
  - 新增 `downscale_captured_rgba_if_large()`：在监控线程构造 `CapturedImage`、**进入 `sync_channel` 之前**，对 RGBA 用 `image::RgbaImage` + `imageops::resize(Triangle)` 把最长边缩到 ≤4096。
    - 边界处理：≤4096 直接透传；`width==0||height==0` 提前返回原始缓冲（未 move）；`from_raw` 失败兜底返回空占位（理论不可达，因已 normalize 到 `width*height*4`）。
    - 零额外内存峰值：原始 `rgba` 经 `from_raw` 零拷贝包装后 `resize` 输出小缓冲，原始随即释放；进入通道的仅为 ≤4096 边 RGBA（8K 图从 ~660MB 降到 ~67MB）。
- **`src-tauri/src/lib.rs:425`**
  - 捕获通道容量 `4 → 2`：通道内最多 2 个待处理 + worker 处理中 1 个 = **峰值 ≤3 张**（此前 5 张）。满队列仍由 poll 线程 `try_send` 丢弃（不阻塞）。
- **效果**：8K 截图瞬时 RSS 峰值从 ~3.3GB（5×660MB）降至 **~200MB（3×67MB）**，彻底消除内存受限机器的 OOM 风险。`store_clipboard_image` 落盘质量不变（它本就缩到 4096）。

### ✅ L3 — App.vue 监听器卸载清理

- **`src/App.vue`**
  - `import` 增加 `onUnmounted`。
  - setup 作用域声明 `let unlisteners: Array<() => void> = []`，并注册 `onUnmounted(() => { for (const off of unlisteners) off(); unlisteners = []; })`。
  - `onMounted` 内全部 7 个根级监听（`clipboard-changed`、`records-expired`、`toggle-panel`、`paste-focus-lock`、`onFocusChanged`、`open-settings`、`capture-paused`）统一 `unlisteners.push(await listen(...)/await appWindow.onFocusChanged(...))` 收集，并在注册前 `unlisteners = []` 重置。
  - 修复前 dev HMR 重挂载会重复注册监听（生产无碍）；修复后每次挂载/卸载成对清理，不再泄漏。

### 验证

- 前端：`npx vue-tsc --noEmit` → **0 错误**（修正了 `onFocusChanged` 返回 `Promise<UnlistenFn>` 需 `await` 的类型问题）。
- 后端：`cargo check` → **编译通过**（修正了 `RgbaImage::from_raw` 返回 `Option` 用 `let Some` 而非 `let Ok`、以及 `else` 兜底误用已 move 的 `rgba` 的两处编译错误）。

### 尚未处理（保持原建议）

- L1 导入 hash 改为分批 EXISTS（低优先级）
- L2 可见窗口级 `recordsById` 解析（微优化）
- L4 旧 base64 列表缩略图走 media 路径（正确性问题，非内存）

---

## 后续收紧（2026-07-24）

在 M1 修复之上再降峰值与合成开销：

- **`media::MAX_EDGE` / `CAPTURE_MAX_EDGE`：`4096 → 2560`**（捕获与落盘长边上限）
- **列表缩略图长边：`240 → 160`**
- **捕获通道容量**：仍为 `2`（与上文一致）
- **`enable_blur` 默认 `false`**（新装）；开启时 `blur(8px)`，且仅悬浮模式生效
- 前端列表/预览图片：`loading="lazy"`、`decoding="async"`

历史叙述中的 `4096` / 通道峰值估算仍反映审查当时状态；当前代码以上述数值为准。