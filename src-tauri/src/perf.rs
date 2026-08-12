//! Opt-in critical-path timing instrumentation.
//!
//! All timings are `tracing::debug!` records under the `perf` target. The
//! default log filter is `info`, so these are filtered out (and effectively
//! free) unless profiling with `RUST_LOG=perf=debug` — see `docs/perf.md`.

use std::time::Instant;

/// Log the elapsed time since `start` for a named hot-path step.
#[inline]
pub(crate) fn log_elapsed(name: &'static str, start: Instant) {
    tracing::debug!(target: "perf", name, elapsed_ms = start.elapsed().as_millis() as u64);
}
