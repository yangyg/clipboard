# 代码审查报告 — 2026-08-08

## 已修复跟踪

| 编号 | 状态 | 提交 |
|------|------|------|
| H1 / H2 / H3 / H4、M1 / M2 / M3 / M4 | ✅ 已修复 | `07610a4` |
| M5 / M6 / M7 / M9 / M11 | ✅ 已修复 | 本批 |
| M8 / M10、WebDAV 凭据、导入前置校验、M12 | ⏳ 待做 | 见下文 |

本轮对全代码库（前端 Vue3/TS + Rust/Tauri 后端）做了全面审查，覆盖：代码质量、潜在漏洞、性能瓶颈、安全隐患、最佳实践偏离。结论按严重程度分级如下。

## 高优先级

| # | 位置 | 类别 | 问题 |
|---|------|------|------|
| H1 | `src-tauri/src/clipboard/monitor.rs:350-367` | 正确性/健壮性 | `is_primarily_url` 把在 `t.to_lowercase()` 上的字节索引用来切片**原字符串** `t[..start]` / `&t[start..]`。`to_lowercase()` 可能改变字节长度（如 `ẞ`→`ss`、`İ`→`i̇`），此时 `start` 落在非字符边界上，**panic** 会终止轮询线程且无 `catch_unwind`——剪贴板采集在下次重启前永久停止。 |
| H2 | `src-tauri/src/media.rs:20-30`（经 `clipboard/image.rs:42-55`） | 安全（本地 DoS） | `normalize_rgba_len` 按**未经验证**的 DIB 头部 `width×height×4` 计算 `expected`，当缓冲区偏短时 `rgba.resize(expected, 0)`。本机任一行程放一个伪造尺寸的小位图到剪贴板，可触发 ~17GB 分配导致进程 OOM 中止。 |
| H3 | `src-tauri/src/webdav/client.rs:47-51` | 安全（SSRF / 凭据降级泄露） | reqwest `Client` 使用默认重定向策略（可跨 scheme/host 跟随 10 跳）。服务器可 301→`http://` 同 host（Basic Auth 头随明文重发）或 307/308 重定向到内网地址（`127.0.0.1`、云元数据等），剪贴板 bundle 会被发到攻击者可影响的目标。 |
| H4 | `src/composables/useConfirm.ts:29-32` | 正确性（静默数据丢失） | 连续两次 `confirm()`：第二次会 `settle(false)` 悄悄把第一个尚未展示的确认对话框以“取消”收场，调用方无从区分是用户取消还是被覆盖。`clearHistory`/`permanentlyDeleteRecord`/`emptyTrash` 都依赖它。 |

## 中优先级

| # | 位置 | 类别 | 问题 |
|---|------|------|------|
| M1 | `src-tauri/src/db/records_write.rs:171-196` + `db/settings.rs:125-149` | 正确性/数据丢失 | 回收站保留期从 `updated_at`（入库更新时间）起算，而 trash 不更新 `updated_at`。30 天前复制、今天入回收站的记录会被立即清除，用户无法在配置窗口内找回。 |
| M2 | `src-tauri/src/db/records_import.rs:175-203` | 正确性 | 导入/WebDAV 拉取 INSERT 漏写 `content_len`（默认 0）。一次性回填（`db/mod.rs:78-87`）只跑一次，此后所有导入记录 `content_len=0` → 统计 `storage_bytes` 低估、列表按长度截断错误。 |
| M3 | `src-tauri/src/db/records_import.rs:77-83,120-133` | 正确性 | 合并去重的 `existing_hashes` 含回收站行；已删除哈希的记录被重新导入时合并进回收站行，活动列表不出现，且不复活。与捕获去重路径（排除回收站）不一致。 |
| M4 | `src/composables/useConfirm.ts`（同上）之后 `src/stores/clipboardRecordActions.ts:104-107` | 正确性 | `togglePin` 对 reactive `records.value` 原地 `.sort()`，绕过引用变更检测，排序抖动与 keyset loadMore 偏移风险。应赋新数组。 |
| M5 | `src/utils/sanitizeHtml.ts:7-15` | 正确性/展示越界 | DOMPurify 缓存键用采样指纹（len/48 抽点 + 首尾 48 字符），短内容或长内容哈希相撞时可能把**另一条**记录的清洗结果当成本条预览显示。**已修复**：改用完整 HTML 做 Map 键（缓存 ≤24 条，内存有界）。 |
| M6 | `src-tauri/src/db/settings.rs:100-123` | 正确性 | 敏感记录 `cleanup_expired` 无条件硬删 `auto_expire_at` 到期行，不尊重 `is_pinned`/`is_favorite`（保留与最大记录数淘汰均尊重这两项）。钉住/收藏的敏感记录到期即被永久删除。**已修复**：到期清理排除已钉住/收藏的行。 |
| M7 | `src-tauri/src/security.rs:14-19` | 安全（加固） | HTML 导入的行内事件处理器黑名单漏掉 `oncanplay/onpointerenter/ondragenter` 等，被断言“安全”的 `content_html` 会在重新粘贴时进入富文本编辑器。**已修复**：补齐 pointer/touch/drag/media 等事件处理器名单 + 回归测试。 |
| M8 | `src-tauri/src/webdav/bundle.rs:98-107` | 同步一致 | 软删除不进推送、无 tombstone，设备 A 删除的行会在设备 B 重新回来。属已文档化的取舍，暂记。 |
| M9 | `src-tauri/src/db/records_search.rs:48-52` | 性能 | 搜索分页只用 OFFSET，页漂移（新行插入导致重复/跳过）；`get_records` 非 `updated_desc` 排序同样回退 OFFSET。**已修复（搜索侧）**：`search_records` 增加 `before_*` keyset 游标，默认 `updated_desc` 排序改用 keyset 分页（命令/契约/前端 loadMore 同步更新）。 |
| M10 | `src-tauri/src/webdav/sync.rs` / `media.rs` | 性能 | 同步将整个 bundle（≤64MB）多次以完整副本驻留内存并按记录 clone；DB 事务、fs、网络请求直接跑在 Tokio executor 而非 `spawn_blocking`。 |
| M11 | `src-tauri/src/media.rs:126-146`（经 `capture.rs`） | 资源泄漏 | 缩略图写入或 DB 插入失败时，已写入的 PNG/缩略图无人引用且不清理，反复失败累积孤立文件。**已修复**：`StoredImage` 增 `created` 标志；缩略图失败时删除刚写的 PNG；DB 插入失败时（仅当本次新建）清理媒体文件。 |

## 低优先级

| # | 位置 | 问题 |
|---|------|------|
| L1 | `db/records_query.rs:149-151` | 行映射 `row.get(...).ok()/.unwrap_or_default()` 静默吞掉列类型/顺序漂移的错误（`RECORD_COLS*` 与映射无测试绑定）。 |
| L2 | `media.rs:252-257` | `media/` 目录缺失时按整个 appdata 根递归算“媒体大小”，统计失真。 |
| L3 | `media.rs:170-181` | `.pending-delete` 隔离文件删除失败后不重试、缓存不更新，累积隐藏文件。 |
| L4 | `clipboard/paste.rs:217-273` | 全局 `SPI_SETFOREGROUNDLOCKTIMEOUT` 改 0 后崩崩溃不还原（`let _ =` 无保护）。 |
| L5 | `clipboard/fgwin.rs:109-177` | 缓存 Mutex 跨磁盘 I/O（读 exe 版本资源）持有，首抓各 app 时拖慢 250ms 轮询。 |
| L6 | `db/tags.rs:113-152` | `get_all_tags` 全表 JOIN 聚合无索引支撑，标签面板刷新慢。 |
| L7 | `db/records_query.rs:31-58` | 1-2 字符搜索整表 `instr` 扫描（已知取舍）。 |
| L8 | `db/tags.rs:316-412` | 删除/重设标签按受影响记录逐条重建 FTS，大标签时数千条语句。 |
| L9 | `composables/useToast.ts:17-30` | 同一 toast 快速重复时不清理旧定时器，快速反馈循环下定时器累积。 |
| L10 | `composables/useClipboardEvents.ts` | `listen()` 逐个 await，任一 reject 则后续事件监听全部不注册。 |
| L11 | `composables/useRecordActions.ts`（媒体） | ——（已由下游修复覆盖，见 git log）|

## 已核实为非问题（供存证）

- SQL 注入：全部用户值走绑定参数；`format!` 只拼常量列名/白名单排序键。
- 路径穿越：媒体相对路径被严格 64 位十六进制哈希正则约束 + canonicalize/前缀检查；导出/导入 JSON 路径拒绝 `..` 与非 `.json`。
- FFI 内存：DPAPI / SetClipboardData 各失败路径均正确释放。
- 死锁：写锁在 `purge_media_pairs` 重新拿读锁前全部释放；无嵌套重入。
- 有界队列：文本/图片 channel 满时丢弃不阻塞轮询线程。
- 剪贴板监控 watermark：占用/抑制窗口期间正确保留（不提交 seq）防丢数据。

## 第二轮修复（2026-08-08，未提交）

基于第二轮全库审查（IPC/数据库/检测/前端/脚本/CI）修复以下问题：

| 编号 | 风险 | 位置 | 修复 |
|------|------|------|------|
| P1-1 | 高 | `src-tauri/src/detect.rs` | 敏感检测误报：password/passwd 改为词边界匹配，`pwd` 与中文「密码/口令」要求赋值标记（`:`/`=`），URL 形态内容跳过关键字规则；验证码仅认可限定词（verification/OTP/auth/security/access/sms/one-time/2FA/验证码/校验码/动态口令），`zip code`/`promo code` 不再命中；补充误报回归测试 |
| P1-2 | 高 | `src-tauri/src/db/records_import.rs` + `commands/import_export.rs` + `webdav/sync.rs` | 导入/WebDAV 元数据消毒：新增 `ImportSanitize`（源自设置），导入时重新检测敏感、重算 `auto_expire_at = now + TTL`，合并路径只 OR 敏感标记不降级；JSON 导入与 WebDAV 拉取均启用 |
| P1-3 | 高 | `src-tauri/src/db/settings.rs` + `records_write.rs` | 过期清理排除回收站行；trash 时清除 `auto_expire_at`，回收站生命周期完全交给 retention；补测试 |
| P2-4 | 中 | `src-tauri/src/db/records_query.rs` | 回收站排序补 `id` tiebreak，与 keyset 谓词严格一致（批量删除同时间戳翻页不再可能丢行） |
| P2-5 | 中 | `src-tauri/src/db/records_write.rs` + `schema_tests.rs` | 去重更新来源时刷新 FTS source 列（内容不变不触发触发器）；补测试 |
| P2-6 | 中 | `src-tauri/src/db/settings.rs` + `security.rs` | DPAPI 解密失败保留密文（不再清空），`save_settings` 防二次加密；凭据不再被静默覆盖为空 |
| P2-8 | 中 | `src/App.vue` | window 模式收到 `toggle-panel:false` 不再重新 `show()` 窗口（托盘隐藏不再被立即撤销） |
| P2-9 | 中 | `src-tauri/src/window.rs` | `apply_window_round_corners` 通过 `GetWindowRgn` 副本释放被替换的旧 region，消除 GDI 句柄泄漏 |
| P2-10 | 中 | `src-tauri/src/webdav/media.rs` | 媒体读写移入 `spawn_blocking`，不再阻塞 Tokio worker |
| P3-1 | 低 | `src/stores/clipboardList.ts` | 新置顶记录插入置顶区顶部而非末尾；detail 缓存增加 8MB 字节上限 |
| P3-2 | 低 | `src/composables/useClipboardEvents.ts` | 事件监听改用 `Promise.allSettled`，单个监听失败不再吞掉其余注册 |
| P3-5 | 低 | `src-tauri/src/security.rs` | `link_scheme` 在解析前后都拒绝含空白/控制字符的内容（`Url::parse` 对空格宽容），补测试 |
| P3-6 | 低 | `src-tauri/src/commands/tags.rs` + `db/mod.rs` | 标签颜色在 IPC 边界归一化到 12 色轮，杜绝 CSS 注入面 |
| P3-7 | 低 | `scripts/doctor.mjs` | sqlite3 增加 `.timeout 5000`（应用运行中不再假失败）；semver 解析容忍预发布后缀 |
| P3-10 | 低 | `src-tauri/src/clipboard/paste.rs` | 前台锁超时只在读取成功时还原，避免崩溃/失败时系统级设置被写 0 持久化 |
| P3-11 | 低 | `src-tauri/src/media.rs` | `media/` 目录缺失时统计返回 0，不再把整个 appdata 根当媒体 |

仍需运行验证：P2-7（paste 图片自写基线与 arboard RGBA 往返保真度，透明通道图片可能不匹配）。

验证基线：`cargo test` 101 通过、`cargo clippy -D warnings` 通过、`cargo fmt --check` 通过、Vitest 164 通过、typecheck/lint/build/check:ipc-contract/check:schema 全部通过。

## 第三轮收尾（2026-08-08，未提交）

| 编号 | 风险 | 位置 | 修复 |
|------|------|------|------|
| P3-3 | 低 | `src-tauri/src/db/records_query.rs` + `schema_tests.rs` | `map_record_row` 去除 `content_len/alias/source_name` 的静默兜底（三列均 NOT NULL），列序/类型漂移改为大声失败；新增双列清单（RECORD_COLS / RECORD_COLS_LIST）位置绑定回归测试 |
| P3-8 | 低 | `scripts/check-schema.mjs` | 新增 RECORD_COLS 与 RECORD_COLS_LIST 逐位置列名序列校验；`extractColNames` 改为按顶层逗号切分，正确处理 `substr(content, 1, 400)` 类括号表达式 |
| 启动健壮性 | 低 | `src-tauri/src/db/mod.rs` + `schema_tests.rs` | `ensure_fts` 提前到迁移之前执行——legacy 库缺 `records_fts`/`fts_version` 时，`migrate_text_hash_v2` 的 `refresh_record_fts` 与删除触发器不再因表缺失导致 `ClipboardDb::new` 失败、应用无法启动；补 legacy 无 FTS 库启动回归测试 |

验证基线：`npm run validate` 全绿（Rust 103 用例，前端 164 用例，lint/typecheck/fmt/clippy/契约/构建全部通过）。
