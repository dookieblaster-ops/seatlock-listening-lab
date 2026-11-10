use serde::{Deserialize, Serialize};

#[cfg(feature = "typescript")]
use ts_rs::TS;

/// A validation or warning item with a stable code and a field path in the design file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TS))]
#[cfg_attr(
    feature = "typescript",
    ts(export, export_to = "../../../../web/src/generated/Issue.ts")
)]
pub struct Issue {
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
            code: code.into(),
            message: message.into(),
            field: field.into(),
        }
    }
}
