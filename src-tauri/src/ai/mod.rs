//! AI enrichment: OpenAI-compatible chat-completions client + a worker thread
//! that turns new records into an `alias` (summary) and auto-tags.
//!
//! Privacy rules enforced here (never skipped):
//! - sensitive records are never sent to the model;
//! - only text-ish content types (`text` / `code` / `link`) are eligible;
//! - content is truncated to `ai_max_chars` before leaving the machine,
//!   and the on-disk API key is DPAPI-encrypted (see `db/settings.rs`).
//!
//! The worker never runs on the capture hot path: capture only enqueues a
//! small job and a full queue simply drops it (mirroring the image worker).

pub(crate) mod client;
pub(crate) mod worker;

pub(crate) use client::AiClient;
pub(crate) use worker::{ai_eligible_type, start_ai_worker, AiConfig, AiEnrichJob, AiJobSender};
