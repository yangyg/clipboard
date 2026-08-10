//! AI enrichment worker: consumes enqueued capture jobs, calls the model off
//! the capture hot path, then writes the alias + tags and re-emits the list
//! payload so the frontend refreshes in place.

use std::sync::mpsc;
use std::sync::Arc;
use tauri::Emitter;
use tracing::{info, warn};

use crate::ai::AiClient;
use crate::db::ClipboardDb;
use crate::panel::list_ipc_payload;
use crate::Settings;

/// Capacity: at most 8 queued jobs + 1 in-flight. A full queue drops the event
/// (the poll/capture threads must never block on AI, mirroring the image worker).
const AI_QUEUE_CAPACITY: usize = 8;

pub(crate) type AiJobSender = mpsc::SyncSender<AiEnrichJob>;

/// Which content flavors may be sent to the model. Images/files carry no
/// summarizable text (their `content` is a label or a path).
pub(crate) fn ai_eligible_type(content_type: &str) -> bool {
    matches!(content_type, "text" | "code" | "link")
}

/// Snapshot of the AI-related settings at enqueue time. Keeps the worker
/// isolated from concurrent settings changes mid-flight.
#[derive(Clone)]
pub(crate) struct AiConfig {
    pub summary_enabled: bool,
    pub tags_enabled: bool,
    pub max_chars: usize,
    pub language: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl AiConfig {
    pub(crate) fn from_settings(s: &Settings) -> Self {
        Self {
            summary_enabled: s.ai_summary_alias,
            tags_enabled: s.ai_auto_tag,
            max_chars: s.ai_max_chars.max(0) as usize,
            language: s.language.clone(),
            base_url: s.ai_base_url.clone(),
            api_key: s.ai_api_key.clone(),
            model: s.ai_model.clone(),
        }
    }

    pub(crate) fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

pub(crate) struct AiEnrichJob {
    pub record_id: i64,
    pub content: String,
    pub config: AiConfig,
}

/// Spawn the AI worker and hand back the enqueue handle. Call once at startup
/// (`setup.rs`); the returned sender is threaded into the capture pipeline.
pub(crate) fn start_ai_worker(app: tauri::AppHandle, db: Arc<ClipboardDb>) -> AiJobSender {
    let (tx, rx) = mpsc::sync_channel::<AiEnrichJob>(AI_QUEUE_CAPACITY);
    std::thread::spawn(move || {
        while let Ok(job) = rx.recv() {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_ai_job(&job, &db, &app);
            }))
            .is_err()
            {
                warn!("AI worker recovered from panic");
            }
        }
    });
    tx
}

/// Keep the queue light and the exfil surface minimal: truncate before enqueue.
fn capped_content(content: &str, max_chars: usize) -> String {
    let n = max_chars.max(64);
    if content.chars().count() <= n {
        content.to_string()
    } else {
        content.chars().take(n).collect()
    }
}

fn process_ai_job(job: &AiEnrichJob, db: &ClipboardDb, app: &tauri::AppHandle) {
    // Live re-check: the user may have turned AI off / moved provider while the
    // job sat in the queue. Never call out in that state.
    let live = match db.get_settings() {
        Ok(s) => (*s).clone(),
        Err(e) => {
            warn!("Failed to load settings in AI worker: {}", e);
            return;
        }
    };
    if !live.features.ai || !live.enable_ai {
        return;
    }

    // The record may have been deleted / trashed since enqueue.
    let Some(record) = db.get_record(job.record_id).ok().flatten() else {
        return;
    };
    if record.is_sensitive || record.is_trashed {
        return;
    }

    let client = match AiClient::new(&job.config.base_url, &job.config.api_key, &job.config.model) {
        Ok(c) => c,
        Err(e) => {
            warn!("AI client config rejected: {}", e);
            return;
        }
    };

    let result = match tauri::async_runtime::block_on(client.chat_json(
        &capped_content(&job.content, job.config.max_chars),
        &job.config.language,
    )) {
        Ok(r) => r,
        Err(e) => {
            warn!("AI enrichment failed for record {}: {}", job.record_id, e);
            return;
        }
    };

    let mut changed = false;
    if job.config.summary_enabled && !result.summary.is_empty() {
        // Ownership guard: only write the alias while it is still empty, so a
        // user-edited alias (or an earlier AI summary) is never overwritten.
        if let Ok(Some(current)) = db.get_record(job.record_id) {
            if current.alias.trim().is_empty() && !current.is_trashed {
                match db.set_record_alias(job.record_id, &result.summary) {
                    Ok(written) => changed |= !written.is_empty(),
                    Err(e) => warn!(
                        "Failed to write AI alias for record {}: {}",
                        job.record_id, e
                    ),
                }
            }
        }
    }
    if job.config.tags_enabled && !result.tags.is_empty() {
        match db.add_auto_tags_by_name(job.record_id, &result.tags) {
            Ok(n) => changed |= n > 0,
            Err(e) => warn!("Failed to add AI tags for record {}: {}", job.record_id, e),
        }
    }

    if changed {
        if let Ok(Some(rec)) = db.get_record(job.record_id) {
            let _ = app.emit("clipboard-changed", list_ipc_payload(rec));
        }
        info!("AI enrichment applied to record {}", job.record_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn eligible_content_types_only() {
        assert!(ai_eligible_type("text"));
        assert!(ai_eligible_type("code"));
        assert!(ai_eligible_type("link"));
        assert!(!ai_eligible_type("image"));
        assert!(!ai_eligible_type("file"));
    }

    #[test]
    fn capped_content_truncates_and_clamps() {
        assert_eq!(capped_content("abc", 100), "abc");
        let long = "x".repeat(1000);
        assert_eq!(capped_content(&long, 128).chars().count(), 128);
        // max_chars below the clamp still yields something sane.
        assert_eq!(capped_content(&long, 4).chars().count(), 64);
    }

    #[test]
    fn config_requires_url_and_model() {
        let mut s = Settings::default();
        s.enable_ai = true;
        s.ai_base_url = "https://ok.example/v1".into();
        s.ai_model = "m".into();
        let cfg = AiConfig::from_settings(&s);
        assert!(cfg.is_configured());
        s.ai_model.clear();
        assert!(!AiConfig::from_settings(&s).is_configured());
    }
}
