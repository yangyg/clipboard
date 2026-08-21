//! User-initiated on-demand enrichment (preview / context menu).
//!
//! Capture still goes through the fire-and-forget worker (empty-alias only).
//! This path waits for the model, may overwrite an existing alias, and returns
//! a structured outcome so the frontend can patch the list without relying on
//! `clipboard-changed` (which is dropped while capture is paused).

use serde::Serialize;

use crate::ai::worker::{ai_eligible_type, capped_content, AiConfig};
use crate::db::ClipboardDb;
use crate::features::{require_feature, FeatureId};
use crate::types::AiResult;
use crate::{ClipboardRecord, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiEnrichMode {
    Summary,
    Tags,
}

impl AiEnrichMode {
    pub(crate) fn parse(s: &str) -> Result<Self, String> {
        match s {
            "summary" => Ok(Self::Summary),
            "tags" => Ok(Self::Tags),
            _ => Err("mode 必须是 summary 或 tags".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct AiEnrichOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Validate dual-gate / privacy / type rules and return truncated content +
/// the live AI config. Does **not** enforce `ai_min_chars` — a click is consent.
pub(crate) fn prepare_on_demand(
    settings: &Settings,
    record: &ClipboardRecord,
    mode: AiEnrichMode,
) -> Result<(String, AiConfig), String> {
    require_feature(settings, FeatureId::Ai)?;
    if mode == AiEnrichMode::Tags {
        require_feature(settings, FeatureId::Tags)?;
    }
    if !settings.enable_ai {
        return Err("请先在 AI 设置中开启 AI 功能".into());
    }
    let config = AiConfig::from_settings(settings);
    if !config.is_configured() {
        return Err("AI 地址与模型名不能为空".into());
    }
    if record.is_trashed {
        return Err("回收站中的记录无法使用 AI".into());
    }
    if record.is_sensitive {
        return Err("敏感内容不会发送给 AI".into());
    }
    if !ai_eligible_type(&record.content_type) {
        return Err("该类型不支持 AI 富集".into());
    }
    Ok((capped_content(&record.content, config.max_chars), config))
}

/// Apply one side of an `AiResult`. Empty summary never writes (would wipe a
/// user alias). Tags are additive; zero new links is an error so the click
/// has a visible outcome.
pub(crate) fn apply_on_demand(
    db: &ClipboardDb,
    record_id: i64,
    result: &AiResult,
    mode: AiEnrichMode,
) -> Result<AiEnrichOutcome, String> {
    match mode {
        AiEnrichMode::Summary => {
            let summary = result.summary.trim();
            if summary.is_empty() {
                return Err("模型未生成摘要".into());
            }
            let alias = db
                .set_record_alias(record_id, summary)
                .map_err(|e| e.to_string())?;
            Ok(AiEnrichOutcome {
                alias: Some(alias),
                tags: None,
            })
        }
        AiEnrichMode::Tags => {
            if result.tags.iter().all(|t| t.trim().is_empty()) {
                return Err("没有可添加的标签".into());
            }
            let added = db
                .add_auto_tags_by_name(record_id, &result.tags)
                .map_err(|e| e.to_string())?;
            if added == 0 {
                return Err("没有可添加的标签".into());
            }
            let tags = db
                .get_record_tag_names(record_id)
                .map_err(|e| e.to_string())?;
            Ok(AiEnrichOutcome {
                alias: None,
                tags: Some(tags),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ClipboardDb, ContentType};
    use crate::detect::sha256_hash;

    fn ready_settings() -> Settings {
        let mut s = Settings::default();
        s.enable_ai = true;
        s
    }

    fn sample(content_type: &str) -> ClipboardRecord {
        ClipboardRecord {
            id: 1,
            content: "hello world this is clipboard text".into(),
            content_type: content_type.into(),
            source_app: "app.exe".into(),
            source_window: "win".into(),
            source_name: String::new(),
            source_device_id: String::new(),
            hash: "h".into(),
            copy_count: 0,
            is_favorite: false,
            is_pinned: false,
            is_sensitive: false,
            is_trashed: false,
            auto_expire_at: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            tags: vec![],
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

    fn temp_db() -> (ClipboardDb, std::path::PathBuf) {
        crate::db::test_util::temp_db("ai_on_demand")
    }

    fn insert(db: &ClipboardDb, content: &str, content_type: &ContentType, sensitive: bool) -> i64 {
        let hash = sha256_hash(&sha256_hash(content));
        db.insert_record(
            content,
            content_type,
            &hash,
            sensitive,
            1000,
            600,
            "app.exe",
            "win",
            "",
            None,
            None,
        )
        .unwrap()
        .0
    }

    #[test]
    fn parse_mode_accepts_only_summary_and_tags() {
        assert_eq!(
            AiEnrichMode::parse("summary").unwrap(),
            AiEnrichMode::Summary
        );
        assert_eq!(AiEnrichMode::parse("tags").unwrap(), AiEnrichMode::Tags);
        assert!(AiEnrichMode::parse("rewrite").is_err());
    }

    #[test]
    fn prepare_rejects_when_runtime_or_capability_off() {
        let rec = sample("text");
        let mut s = ready_settings();
        s.enable_ai = false;
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary)
            .unwrap_err()
            .contains("开启 AI"));

        s.enable_ai = true;
        s.features.ai = false;
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary)
            .unwrap_err()
            .contains("feature disabled: ai"));
    }

    #[test]
    fn prepare_tags_requires_tags_capability() {
        let rec = sample("text");
        let mut s = ready_settings();
        s.features.tags = false;
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Tags)
            .unwrap_err()
            .contains("feature disabled: tags"));
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary).is_ok());
    }

    #[test]
    fn prepare_rejects_sensitive_image_and_trashed() {
        let s = ready_settings();
        let mut rec = sample("text");
        rec.is_sensitive = true;
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary)
            .unwrap_err()
            .contains("敏感"));

        rec.is_sensitive = false;
        rec.content_type = "image".into();
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary)
            .unwrap_err()
            .contains("类型"));

        rec.content_type = "text".into();
        rec.is_trashed = true;
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary)
            .unwrap_err()
            .contains("回收站"));
    }

    #[test]
    fn prepare_ignores_min_chars_and_truncates_to_max() {
        let mut s = ready_settings();
        s.ai_min_chars = 500;
        s.ai_max_chars = 80;
        let mut rec = sample("text");
        rec.content = "短".into();
        assert!(prepare_on_demand(&s, &rec, AiEnrichMode::Summary).is_ok());

        rec.content = "字".repeat(200);
        let (capped, _) = prepare_on_demand(&s, &rec, AiEnrichMode::Summary).unwrap();
        assert_eq!(capped.chars().count(), 80);
    }

    #[test]
    fn apply_summary_overwrites_existing_alias() {
        let (db, dir) = temp_db();
        let id = insert(&db, "payload for summary", &ContentType::Text, false);
        db.set_record_alias(id, "old alias").unwrap();
        let out = apply_on_demand(
            &db,
            id,
            &AiResult {
                summary: "  new title  ".into(),
                tags: vec![],
            },
            AiEnrichMode::Summary,
        )
        .unwrap();
        assert_eq!(out.alias.as_deref(), Some("new title"));
        let rec = db.get_record(id).unwrap().unwrap();
        assert_eq!(rec.alias, "new title");
        crate::db::test_util::cleanup(dir);
    }

    #[test]
    fn apply_empty_summary_does_not_clear_alias() {
        let (db, dir) = temp_db();
        let id = insert(&db, "payload keep alias", &ContentType::Text, false);
        db.set_record_alias(id, "keep me").unwrap();
        let err = apply_on_demand(
            &db,
            id,
            &AiResult {
                summary: "   ".into(),
                tags: vec!["x".into()],
            },
            AiEnrichMode::Summary,
        )
        .unwrap_err();
        assert!(err.contains("未生成摘要"));
        let rec = db.get_record(id).unwrap().unwrap();
        assert_eq!(rec.alias, "keep me");
        crate::db::test_util::cleanup(dir);
    }

    #[test]
    fn apply_tags_is_additive_and_errors_when_nothing_new() {
        let (db, dir) = temp_db();
        let id = insert(&db, "payload for tags", &ContentType::Text, false);
        let first = apply_on_demand(
            &db,
            id,
            &AiResult {
                summary: String::new(),
                tags: vec!["前端".into(), "Vue".into()],
            },
            AiEnrichMode::Tags,
        )
        .unwrap();
        let tags = first.tags.unwrap();
        assert!(tags.iter().any(|t| t == "前端"));
        assert!(tags.iter().any(|t| t == "Vue"));

        let err = apply_on_demand(
            &db,
            id,
            &AiResult {
                summary: String::new(),
                tags: vec!["前端".into(), "Vue".into()],
            },
            AiEnrichMode::Tags,
        )
        .unwrap_err();
        assert!(err.contains("没有可添加的标签"));

        let second = apply_on_demand(
            &db,
            id,
            &AiResult {
                summary: String::new(),
                tags: vec!["前端".into(), "部署".into()],
            },
            AiEnrichMode::Tags,
        )
        .unwrap();
        let tags = second.tags.unwrap();
        assert!(tags.iter().any(|t| t == "前端"));
        assert!(tags.iter().any(|t| t == "Vue"));
        assert!(tags.iter().any(|t| t == "部署"));
        crate::db::test_util::cleanup(dir);
    }
}
