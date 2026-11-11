use serde_json::Value;

use crate::design::DesignFile;

include!(concat!(env!("OUT_DIR"), "/embedded_catalog.rs"));

pub fn embedded_file(path: &str) -> Option<&'static str> {
    EMBEDDED
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, text)| *text)
}

pub fn driver(id: &str) -> Option<Value> {
    json_with_id("catalog/drivers/", id)
}

pub fn amplifier(id: &str) -> Option<Value> {
    json_with_id("catalog/electronics/", id)
}

pub fn stock(id: &str) -> Option<Value> {
    json_with_id("catalog/stock/", id)
}

/// The owner's printer: the record named by `id`, or the first one in `catalog/printers/`.
pub fn printer(id: Option<&str>) -> Option<Value> {
    if let Some(id) = id {
        return json_with_id("catalog/printers/", id);
    }
    EMBEDDED
        .iter()
        .filter(|(path, _)| path.starts_with("catalog/printers/") && path.ends_with(".json"))
        .find_map(|(_, text)| serde_json::from_str::<Value>(text).ok())
}

pub fn material(name: &str) -> Option<Value> {
    let text = embedded_file("catalog/materials.json")?;
    let value: Value = serde_json::from_str(text).ok()?;
    value.get(name).cloned()
}

pub fn target(name: &str) -> Option<Value> {
    let path = format!("targets/{name}.json");
    let text = embedded_file(&path)?;
    serde_json::from_str(text).ok()
}

fn json_with_id(prefix: &str, id: &str) -> Option<Value> {
    for (path, text) in EMBEDDED {
        if path.starts_with(prefix) && path.ends_with(".json") {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                if value.get("id").and_then(|item| item.as_str()) == Some(id) {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Prices stay blank until a catalog record has one. Never invent a number.
pub fn cost_note(design: &DesignFile) -> String {
    let mut unknown = false;
    let mut unconfirmed = false;
    for radiator in &design.speaker.radiators {
        if radiator.kind != "cone" {
            continue;
        }
        let Some(id) = radiator
            .fields
            .get("driver_id")
            .and_then(|value| value.as_str())
        else {
            unknown = true;
            continue;
        };
        match driver(id) {
            Some(record) => {
                if record.get("price").is_none_or(|price| price.is_null()) {
                    unknown = true;
                } else if record
                    .pointer("/price/via")
                    .and_then(|value| value.as_str())
                    .is_some()
                    || record
                        .pointer("/mounting/via")
                        .and_then(|value| value.as_str())
                        .is_some()
                {
                    unconfirmed = true;
                }
            }
            None => unknown = true,
        }
    }
    for amp_id in &design.electronics.amplifiers {
        match amplifier(amp_id) {
            Some(record) => {
                if record.get("price").is_none_or(|price| price.is_null()) {
                    unknown = true;
                } else if record
                    .pointer("/price/via")
                    .and_then(|value| value.as_str())
                    .is_some()
                {
                    unconfirmed = true;
                }
            }
            None => unknown = true,
        }
    }
    if let Some(stock_id) = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("stock_id")
        .and_then(|value| value.as_str())
    {
        match stock(stock_id) {
            Some(record) => {
                if record.get("price").is_none_or(|price| price.is_null()) {
                    unknown = true;
                } else if record
                    .pointer("/price/via")
                    .and_then(|value| value.as_str())
                    .is_some()
                {
                    unconfirmed = true;
                }
            }
            None => unknown = true,
        }
    }
    match material(&design.build.material) {
        Some(record) => {
            if record
                .get("price_per_kg")
                .is_none_or(|price| price.is_null())
            {
                unknown = true;
            }
        }
        None => unknown = true,
    }
    if unknown && unconfirmed {
        "price unknown, and one figure is unconfirmed".to_string()
    } else if unknown {
        "price unknown".to_string()
    } else if unconfirmed {
        "priced, includes an unconfirmed figure".to_string()
    } else {
        "priced".to_string()
    }
}
