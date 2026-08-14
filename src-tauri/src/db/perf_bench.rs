//! Release-only performance benchmarks (ignored by the default test run).
//!
//! Seeds a temp DB with 50k text + 5k image records and reports p50/p95 for
//! the hot query/write paths. Baseline and target tables live in
//! `docs/perf.md`; re-run after optimizations to verify improvements.
//!
//! Run:
//!   cargo test --release --manifest-path src-tauri/Cargo.toml -- --ignored perf --nocapture

use std::time::Instant;

use super::{ClipboardDb, ContentType, PageCursor};
use crate::media;

const TEXT_ROWS: usize = 50_000;
const IMAGE_ROWS: usize = 5_000;
const TAG_NAMES: &[&str] = &[
    "部署", "前端", "链接", "重要", "设计", "测试", "文档", "代码", "图片", "备忘",
];

fn temp_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "clipvault_perf_bench_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn report(name: &str, samples: &[u64]) {
    if samples.is_empty() {
        return;
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    let pct = |q: f64| -> u64 {
        let idx = ((v.len() as f64) * q).ceil().max(1.0) as usize - 1;
        v[idx.min(v.len() - 1)]
    };
    let sum: u64 = v.iter().sum();
    let avg = sum as f64 / v.len() as f64;
    println!(
        "{name:<26} n={:>4}  p50={:>8.2}ms  p95={:>8.2}ms  avg={:>8.2}ms",
        v.len(),
        pct(0.50) as f64 / 1000.0,
        pct(0.95) as f64 / 1000.0,
        avg / 1000.0
    );
}

/// Deterministic mixed text payload (types/sizes mirror real capture loads).
fn text_payload(i: usize) -> (String, &'static str, bool) {
    match i % 10 {
        0..=3 => (
            format!("clipboard note {i} 中文备忘 perfbench-{i}"),
            "text",
            false,
        ),
        4..=5 => (
            format!("fn perfbench_{i}() {{ let x = {i}; return x + 1; }} // code snippet"),
            "code",
            false,
        ),
        6..=7 => (
            format!("https://example.com/item/perfbench/{i}"),
            "link",
            false,
        ),
        8 => (
            format!("长文本 perfbench-{i} {} 末尾", "重复填充内容 ".repeat(40)),
            "text",
            false,
        ),
        _ => (
            format!(
                "perfbench-{i} 密码 password=secret 验证码 {} 请勿泄露",
                1000 + (i % 9000)
            ),
            "text",
            true,
        ),
    }
}

fn text_hash(i: usize) -> String {
    format!("{:064x}", i + 1)
}

fn image_hash(i: usize) -> String {
    format!("{:064x}", 1_000_000 + i)
}

fn seed_rows(db: &ClipboardDb) {
    let now = chrono::Utc::now().to_rfc3339();
    let conn = db.lock_write();
    conn.execute_batch("BEGIN").unwrap();
    for i in 0..TEXT_ROWS {
        let (content, ct, sensitive) = text_payload(i);
        let len = content.chars().count() as i64;
        let source = if i % 10 == 0 {
            "notepad.exe".to_string()
        } else {
            format!("app-{}.exe", i % 200)
        };
        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, source_name,
                                  source_device_id, hash, copy_count, is_favorite, is_pinned,
                                  is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                                  media_path, thumb_path, width, height, content_html, content_len,
                                  alias)
             VALUES (?1, ?2, ?3, '', '', '', ?4, 0, 0, 0, ?5, 0, NULL, ?6, ?6,
                     NULL, NULL, NULL, NULL, NULL, ?7, '')",
            rusqlite::params![
                content,
                ct,
                source,
                text_hash(i),
                sensitive as i32,
                now,
                len,
            ],
        )
        .unwrap();
    }
    for i in 0..IMAGE_ROWS {
        let label = "[image 1280x720]";
        let hash = image_hash(i);
        conn.execute(
            "INSERT INTO records (content, content_type, source_app, source_window, source_name,
                                  source_device_id, hash, copy_count, is_favorite, is_pinned,
                                  is_sensitive, is_trashed, auto_expire_at, created_at, updated_at,
                                  media_path, thumb_path, width, height, content_html, content_len,
                                  alias)
             VALUES (?1, 'image', 'snipping.exe', '', '', '', ?2, 0, 0, 0, 0, 0, NULL, ?3, ?3,
                     ?4, ?5, 1280, 720, NULL, ?6, '')",
            rusqlite::params![
                label,
                hash,
                now,
                format!("media/{hash}.png"),
                format!("media/thumbs/{hash}.jpg"),
                label.chars().count() as i64,
            ],
        )
        .unwrap();
    }
    // Tags + links on ~10% of text rows (5k links) so get_all_tags is non-trivial.
    for name in TAG_NAMES {
        conn.execute(
            "INSERT OR IGNORE INTO tags (name, color, is_auto) VALUES (?1, '#6366f1', 1)",
            [name],
        )
        .unwrap();
    }
    let tag_ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM tags WHERE name IN (SELECT name FROM tags)")
            .unwrap();
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        ids
    };
    for i in 0..TEXT_ROWS / 10 {
        let record_id = i as i64 * 10 + 1;
        let tag_a = tag_ids[i % tag_ids.len()];
        let tag_b = tag_ids[(i * 3 + 1) % tag_ids.len()];
        conn.execute(
            "INSERT OR IGNORE INTO record_tags (record_id, tag_id) VALUES (?1, ?2), (?1, ?3)",
            rusqlite::params![record_id, tag_a, tag_b],
        )
        .unwrap();
    }
    conn.execute_batch("COMMIT").unwrap();
}

fn bench_image_encode(root: &std::path::Path) -> Vec<u64> {
    const W: u32 = 1280;
    const H: u32 = 720;
    let mut samples = Vec::with_capacity(32);
    for i in 0..32u32 {
        let mut rgba = vec![0u8; (W * H * 4) as usize];
        // Deterministic pseudo-random-ish pattern (fast to build).
        let mut seed = i.wrapping_mul(0x9E37_79B9);
        for (idx, chunk) in rgba.chunks_mut(4).enumerate() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (seed >> 24) as u8;
            chunk[0] = v;
            chunk[1] = v.wrapping_add(idx as u8);
            chunk[2] = v.wrapping_sub(idx as u8);
            chunk[3] = 255;
        }
        let start = Instant::now();
        media::store_clipboard_image(root, rgba, W, H, &image_hash(100_000 + i as usize)).unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    samples
}

#[test]
#[ignore = "release-only performance benchmark; run with -- --ignored perf"]
fn perf_queries_50k_5k() {
    let root = temp_root("seed");
    media::ensure_dirs(&root).unwrap();
    let db = ClipboardDb::new(&root.join("bench.db"), root.clone()).unwrap();

    let seed_start = Instant::now();
    seed_rows(&db);
    println!(
        "seed: {} text + {} image rows + tags in {:.2}s",
        TEXT_ROWS,
        IMAGE_ROWS,
        seed_start.elapsed().as_secs_f64()
    );

    // --- list page (keyset default sort) ---
    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        db.get_records(
            60,
            0,
            false,
            None,
            false,
            None,
            Some("updated_desc"),
            PageCursor::default(),
            true,
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("get_records page1", &samples);

    // --- search 3-char (FTS5 trigram; matches all rows) ---
    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        db.search_records(
            "perfbench",
            60,
            0,
            None,
            false,
            None,
            Some("updated_desc"),
            true,
            PageCursor::default(),
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("search 3-char (FTS)", &samples);

    // --- search 3+ char sparse (FTS5; realistic specificity, ~250 hits) ---
    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        db.search_records(
            "app-3",
            60,
            0,
            None,
            false,
            None,
            Some("updated_desc"),
            true,
            PageCursor::default(),
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("search 3-char sparse", &samples);

    // --- search 2-char (instr over content, known tradeoff) ---
    let mut samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let start = Instant::now();
        db.search_records(
            "ne",
            60,
            0,
            None,
            false,
            None,
            Some("updated_desc"),
            true,
            PageCursor::default(),
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("search 2-char (instr)", &samples);

    // --- search 1-char (alias/source/tags only) ---
    let mut samples = Vec::with_capacity(20);
    for _ in 0..20 {
        let start = Instant::now();
        db.search_records(
            "n",
            60,
            0,
            None,
            false,
            None,
            Some("updated_desc"),
            true,
            PageCursor::default(),
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("search 1-char", &samples);

    // --- stats cold + TTL hits ---
    let start = Instant::now();
    db.get_stats().unwrap();
    report("get_stats cold", &[start.elapsed().as_micros() as u64]);
    let mut samples = Vec::with_capacity(5);
    for _ in 0..5 {
        let start = Instant::now();
        db.get_stats().unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("get_stats TTL hit", &samples);

    // --- tags cold + cache hits ---
    let start = Instant::now();
    db.get_all_tags(None, false).unwrap();
    report("get_all_tags cold", &[start.elapsed().as_micros() as u64]);
    let mut samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let start = Instant::now();
        db.get_all_tags(None, false).unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("get_all_tags cache hit", &samples);

    // --- text insert (full capture path incl. FTS trigger) ---
    let mut samples = Vec::with_capacity(100);
    for i in 0..100 {
        let content = format!("perfbench-insert-{i}-{}", "内容 ".repeat(20));
        let hash = crate::detect::sha256_hash(&crate::detect::sha256_hash(&content));
        let start = Instant::now();
        db.insert_record(
            &content,
            &ContentType::Text,
            &hash,
            false,
            300_000,
            0,
            "bench.exe",
            "",
            "",
            None,
            None,
        )
        .unwrap();
        samples.push(start.elapsed().as_micros() as u64);
    }
    report("insert text (FTS)", &samples);

    // --- image encode + store (PNG + thumb) ---
    let encode_root = temp_root("encode");
    media::ensure_dirs(&encode_root).unwrap();
    let samples = bench_image_encode(&encode_root);
    report("image encode+store", &samples);

    // Cleanup temp dirs (best-effort; failures are irrelevant to the bench).
    drop(db);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&encode_root);
}
