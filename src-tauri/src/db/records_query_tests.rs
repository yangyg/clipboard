//! Keyset pagination across list sorts. Lives here so `records_query.rs`
//! stays under the line cap.
use super::{ClipboardDb, PageCursor};
use crate::ClipboardRecord;
use std::path::PathBuf;

fn temp_db() -> (ClipboardDb, PathBuf) {
    super::test_util::temp_db("records_query")
}

fn cleanup(dir: PathBuf) {
    super::test_util::cleanup(dir)
}

fn rec(
    content: &str,
    hash: &str,
    created_at: &str,
    updated_at: &str,
    copy_count: i32,
) -> ClipboardRecord {
    ClipboardRecord {
        id: 0,
        content: content.to_string(),
        content_type: "text".into(),
        source_app: String::new(),
        source_window: String::new(),
        source_name: String::new(),
        source_device_id: String::new(),
        hash: hash.to_string(),
        copy_count,
        is_favorite: false,
        is_pinned: false,
        is_sensitive: false,
        is_trashed: false,
        auto_expire_at: None,
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
        tags: Vec::new(),
        tag_colors: Vec::new(),
        content_html: None,
        media_path: None,
        thumb_path: None,
        width: None,
        height: None,
        media_abs: None,
        thumb_abs: None,
        content_len: None,
        alias: String::new(),
    }
}

fn seed_four(db: &ClipboardDb) {
    db.import_records_with_merge(
        &[
            rec(
                "A",
                "h-a",
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
                1,
            ),
            rec(
                "B",
                "h-b",
                "2026-01-02T00:00:00Z",
                "2026-01-02T00:00:00Z",
                2,
            ),
            rec(
                "C",
                "h-c",
                "2026-01-03T00:00:00Z",
                "2026-01-03T00:00:00Z",
                3,
            ),
            rec(
                "D",
                "h-d",
                "2026-01-04T00:00:00Z",
                "2026-01-04T00:00:00Z",
                4,
            ),
        ],
        100,
        None,
    )
    .unwrap();
}

fn contents(rows: &[ClipboardRecord]) -> Vec<&str> {
    rows.iter().map(|r| r.content.as_str()).collect()
}

fn page1(db: &ClipboardDb, sort: &str) -> Vec<ClipboardRecord> {
    db.get_records(
        2,
        0,
        false,
        None,
        false,
        None,
        Some(sort),
        PageCursor::default(),
        false,
    )
    .unwrap()
}

fn page_after(db: &ClipboardDb, sort: &str, last: &ClipboardRecord) -> Vec<ClipboardRecord> {
    db.get_records(
        2,
        0,
        false,
        None,
        false,
        None,
        Some(sort),
        PageCursor {
            pinned: Some(if last.is_pinned { 1 } else { 0 }),
            updated_at: Some(last.updated_at.as_str()),
            id: Some(last.id),
            created_at: Some(last.created_at.as_str()),
            copy_count: Some(last.copy_count),
        },
        false,
    )
    .unwrap()
}

/// A row inserted at the "top" of `sort` must not make page 2 skip or repeat
/// the records that were already on page 1.
fn assert_stable_page2(sort: &str, incoming: ClipboardRecord) {
    let (db, dir) = temp_db();
    seed_four(&db);
    let first = page1(&db, sort);
    assert_eq!(first.len(), 2);
    let last = &first[1];
    db.import_records_with_merge(&[incoming], 100, None)
        .unwrap();
    let second = page_after(&db, sort, last);
    assert_eq!(contents(&second), ["B", "A"], "sort={sort}");
    cleanup(dir);
}

#[test]
fn created_desc_keyset_ignores_a_newer_insert() {
    assert_stable_page2(
        "created_desc",
        rec(
            "E",
            "h-e",
            "2026-01-05T00:00:00Z",
            "2026-01-05T00:00:00Z",
            0,
        ),
    );
}

#[test]
fn copies_desc_keyset_ignores_a_higher_copy_count_insert() {
    assert_stable_page2(
        "copies_desc",
        rec(
            "E",
            "h-e",
            "2026-01-05T00:00:00Z",
            "2026-01-05T00:00:00Z",
            99,
        ),
    );
}

#[test]
fn updated_asc_keyset_ignores_an_older_insert() {
    // Oldest-first: page 1 is A,B. An even older row would prepend under OFFSET.
    let (db, dir) = temp_db();
    seed_four(&db);
    let first = page1(&db, "updated_asc");
    assert_eq!(contents(&first), ["A", "B"]);
    db.import_records_with_merge(
        &[rec(
            "Z",
            "h-z",
            "2025-12-01T00:00:00Z",
            "2025-12-01T00:00:00Z",
            0,
        )],
        100,
        None,
    )
    .unwrap();
    let second = page_after(&db, "updated_asc", &first[1]);
    assert_eq!(contents(&second), ["C", "D"]);
    cleanup(dir);
}

#[test]
fn updated_desc_keyset_still_pages_after_a_prepend() {
    assert_stable_page2(
        "updated_desc",
        rec(
            "E",
            "h-e",
            "2026-01-05T00:00:00Z",
            "2026-01-05T00:00:00Z",
            0,
        ),
    );
}
