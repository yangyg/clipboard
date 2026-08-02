//! Optional product capabilities. When off: UI hides, capture skips, commands reject.
//! Data is retained; schema stays monolithic.

use crate::Settings;

/// Capability ids — keep in sync with `src/features/capabilities.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureId {
    Tags,
    Batch,
    Sync,
    Stats,
}

impl FeatureId {
    pub fn as_str(self) -> &'static str {
        match self {
            FeatureId::Tags => "tags",
            FeatureId::Batch => "batch",
            FeatureId::Sync => "sync",
            FeatureId::Stats => "stats",
        }
    }
}

/// Per-capability on/off. Missing JSON fields default to `true` (upgrade-safe).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeatureFlags {
    #[serde(default = "default_true")]
    pub tags: bool,
    #[serde(default = "default_true")]
    pub batch: bool,
    #[serde(default = "default_true")]
    pub sync: bool,
    #[serde(default = "default_true")]
    pub stats: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            tags: true,
            batch: true,
            sync: true,
            stats: true,
        }
    }
}

impl FeatureFlags {
    pub fn is_enabled(&self, id: FeatureId) -> bool {
        match id {
            FeatureId::Tags => self.tags,
            FeatureId::Batch => self.batch,
            FeatureId::Sync => self.sync,
            FeatureId::Stats => self.stats,
        }
    }
}

/// Reject a command when the capability is off.
pub fn require_feature(settings: &Settings, id: FeatureId) -> Result<(), String> {
    if settings.features.is_enabled(id) {
        Ok(())
    } else {
        Err(format!("feature disabled: {}", id.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_all_enabled() {
        let f = FeatureFlags::default();
        assert!(f.tags && f.batch && f.sync && f.stats);
    }

    #[test]
    fn require_feature_rejects_when_off() {
        let mut s = Settings::default();
        s.features.tags = false;
        assert!(require_feature(&s, FeatureId::Tags).is_err());
        assert!(require_feature(&s, FeatureId::Batch).is_ok());
    }
}
