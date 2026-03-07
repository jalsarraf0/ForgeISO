use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventPhase {
    Configure,
    Doctor,
    ReleaseLookup,
    Build,
    Scan,
    Test,
    Report,
    Inspect,
    Download,
    Verify,
    Inject,
    Diff,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineEvent {
    pub ts: DateTime<Utc>,
    pub level: EventLevel,
    pub phase: EventPhase,
    pub message: String,
    /// Current operation label shown in the progress panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substage: Option<String>,
    /// Completion percentage 0.0–100.0 when determinable; None = indeterminate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f32>,
    /// Bytes transferred so far (for download/hash operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_done: Option<u64>,
    /// Total bytes expected (for download/hash operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
}

impl EngineEvent {
    pub fn debug(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Debug,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn info(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn warn(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Warn,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    pub fn error(phase: EventPhase, message: impl Into<String>) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Error,
            phase,
            message: message.into(),
            substage: None,
            percent: None,
            bytes_done: None,
            bytes_total: None,
        }
    }

    /// Attach a substage label (fluent builder).
    #[must_use]
    pub fn with_substage(mut self, substage: impl Into<String>) -> Self {
        self.substage = Some(substage.into());
        self
    }

    /// Attach a completion percent 0–100 (fluent builder).
    #[must_use]
    pub fn with_percent(mut self, percent: f32) -> Self {
        self.percent = Some(percent.clamp(0.0, 100.0));
        self
    }

    /// Attach byte transfer progress and auto-compute percent (fluent builder).
    #[must_use]
    pub fn with_bytes(mut self, done: u64, total: u64) -> Self {
        self.bytes_done = Some(done);
        self.bytes_total = Some(total);
        if total > 0 {
            self.percent = Some((done as f32 / total as f32 * 100.0).clamp(0.0, 100.0));
        }
        self
    }

    /// Convenience: structured progress event for a named substage.
    pub fn progress(
        phase: EventPhase,
        substage: impl Into<String>,
        message: impl Into<String>,
        percent: Option<f32>,
    ) -> Self {
        Self {
            ts: Utc::now(),
            level: EventLevel::Info,
            phase,
            message: message.into(),
            substage: Some(substage.into()),
            percent,
            bytes_done: None,
            bytes_total: None,
        }
    }
}
