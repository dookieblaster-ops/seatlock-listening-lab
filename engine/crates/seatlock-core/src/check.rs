use serde_json::Value;

use crate::issue::Issue;
use crate::kinds::KindRegistry;
use crate::DesignFile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub issues: Vec<Issue>,
    pub canonical_json: String,
}

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn design_check(text: &str) -> Result<CheckResult, String> {
    let trimmed = text.trim();
    let value: Value = serde_json::from_str(trimmed).map_err(|e| format!("Invalid JSON: {e}"))?;

    let mut issues = Vec::new();

    if value.get("schema_version").is_none() {
        issues.push(Issue::warning(
            "ERR_SCHEMA",
            "The design file is missing schema_version.",
            "schema_version",
        ));
    }

    match serde_json::from_value::<DesignFile>(value.clone()) {
        Ok(_) => {}
        Err(e) => issues.push(Issue::warning(
            "ERR_SHAPE",
            format!("The design does not match the expected shape: {e}"),
            "",
        )),
    }

    let registry = KindRegistry::level0();
    walk_kinds(&value, "", &registry, &mut issues);

    let has_blocking = issues.iter().any(|i| i.code.starts_with("ERR_"));
    let canonical_json = if has_blocking {
        serde_json::to_string(&value).map_err(|e| e.to_string())?
    } else {
        trimmed.to_string()
    };

    Ok(CheckResult {
        issues,
        canonical_json,
    })
}

fn walk_kinds(value: &Value, path: &str, registry: &KindRegistry, issues: &mut Vec<Issue>) {
    match value {
        Value::Object(map) => {
            if let Some(kind) = map.get("kind").and_then(|k| k.as_str()) {
                if let Some(issue) = classify_kind(path, kind, registry) {
                    issues.push(issue);
                }
            }
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk_kinds(child, &child_path, registry, issues);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                walk_kinds(child, &child_path, registry, issues);
            }
        }
        _ => {}
    }
}

fn classify_kind(path: &str, kind: &str, registry: &KindRegistry) -> Option<Issue> {
    let (family, known) = if path.ends_with("enclosure.outer") || path.ends_with(".outer") {
        ("outer shape", registry.is_known_outer_shape(kind))
    } else if path.contains("radiators") {
        ("radiator", registry.is_known_radiator(kind))
    } else if path.contains("blocks") || path.contains("whole_speaker") {
        ("DSP block", registry.is_known_dsp_block(kind))
    } else if path.contains("room") || path.ends_with("listening") {
        ("room", registry.is_known_room(kind))
    } else {
        return None;
    };

    if known {
        None
    } else {
        Some(Issue::warning(
            "WARN_UNSUPPORTED_KIND",
            format!(
                "This build does not simulate the {family} kind \"{kind}\" yet. The part is kept in the file, but it is left out of results."
            ),
            path.to_string(),
        ))
    }
}
