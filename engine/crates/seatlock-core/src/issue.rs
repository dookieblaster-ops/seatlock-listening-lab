use serde::{Deserialize, Serialize};

#[cfg(feature = "typescript")]
use ts_rs::TS;

/// How serious a design or simulation notice is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/Severity.ts")
)]
pub enum Severity {
    Warning,
    Error,
}

/// A validation or simulation notice with a stable code and a field path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/Issue.ts")
)]
pub struct Issue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub field: String,
}

impl Issue {
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            field: field.into(),
        }
    }

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            field: field.into(),
        }
    }
}
