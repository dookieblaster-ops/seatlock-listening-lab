//! Scores beside costs, the cheap-against-reference comparison (README section 9.2), and the
//! same cabinet built in different ways (README section 6.2).

use serde::Serialize;
use serde_json::{json, Map, Value};

use seatlock_core::design::KindPart;
use seatlock_core::registry::Registry;
use seatlock_core::{catalog, DesignFile, Severity};

use crate::{correction_plan_with, simulate, SimulateRequest, SimulateResult};

/// What the page asks for: `baffle` radiation (README section 14).
pub const RADIATION: &str = "baffle";
/// The everyday enclosure model: `waveguide`, as fast as `lumped` and the same for a box
/// without ducts, but it knows lines, tapers, and pipe resonances (README 5.4).
pub const ENCLOSURE: &str = "waveguide";
pub const SPACE: &str = "half";

/// One design's score and cost, after correction (README section 11).
#[derive(Debug, Clone, Serialize)]
pub struct Score {
    pub f6_hz: f64,
    pub flatness_db: f64,
    pub max_spl_db: f64,
    /// Dollars for the whole system: the sum of the lines that have a price.
    pub cost_usd: f64,
    /// False when a part has no price, so `cost_usd` is a lower bound.
    pub cost_complete: bool,
    pub cost_summary: String,
    pub meets_target: bool,
    /// README section 11.2; 0 when the target is met.
    pub shortfall: f64,
    pub interior_volume_m3: Option<f64>,
    /// Problems that make the score meaningless (a driver that cannot be simulated, say).
    pub problems: Vec<String>,
    /// Olive preference rating from CTA-2034 curves (README 2.6); the ranking objective.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preference: Option<f64>,
}

/// The target's thresholds (README section 11.2).
#[derive(Debug, Clone, Copy)]
pub struct Target {
    /// Minimum Olive preference rating (higher is better).
    pub preference_min: f64,
    /// Kept for correction and reporting; not part of the ranked shortfall.
    pub f6_hz_max: f64,
    /// Kept for correction and reporting; not part of the ranked shortfall.
    pub flatness_db_max: f64,
    pub max_spl_db_min: f64,
    pub budget_max: Option<f64>,
}

impl Target {
    pub fn of(design: &DesignFile) -> Target {
        let record = catalog::target(&design.target).unwrap_or(Value::Null);
        let number =
            |key: &str, default: f64| record.get(key).and_then(Value::as_f64).unwrap_or(default);
        Target {
            preference_min: number("preference_min", 5.0),
            f6_hz_max: number("f6_hz_max", 50.0),
            flatness_db_max: number("flatness_db_max", 1.5),
            max_spl_db_min: number("max_spl_db_min", 95.0),
            budget_max: record.get("budget_max").and_then(Value::as_f64),
        }
    }

    /// Shortfall for ranking (README 11.2): preference deficit plus clean-level deficit.
    /// Bass and evenness are already inside Olive's preference (LFX, NBD, SM).
    pub fn shortfall(&self, preference: Option<f64>, max_spl_db: f64) -> f64 {
        let pref = match preference {
            Some(p) if p.is_finite() => (self.preference_min - p).max(0.0),
            _ => 10.0,
        };
        let spl = if max_spl_db.is_finite() {
            (self.max_spl_db_min - max_spl_db).max(0.0) / 6.0
        } else {
            10.0
        };
        pref + spl
    }
}

/// The design with the correction the page's "Apply correction" button would install: earlier
/// correction blocks are replaced by the engine's plan (README section 8).
pub fn apply_correction(design: &DesignFile, registry: &Registry) -> DesignFile {
    let plan = correction_plan_with(design, registry, RADIATION, SPACE);
    let mut next = design.clone();
    let entries: Vec<Value> = match plan.get("per_radiator").and_then(Value::as_array) {
        Some(list) => list.clone(),
        None => vec![json!({ "filters": plan.get("filters").cloned().unwrap_or(json!([])) })],
    };
    for (index, entry) in entries.iter().enumerate() {
        let Some(chain) = next.dsp.per_radiator.get_mut(index) else {
            continue;
        };
        chain
            .blocks
            .retain(|block| block.fields.get("correction") != Some(&Value::Bool(true)));
        for filter in entry
            .get("filters")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Ok(block) = serde_json::from_value::<KindPart>(filter.clone()) {
                chain.blocks.push(block);
            }
        }
    }
    next.dsp.correction.enabled = true;
    next
}

fn request() -> SimulateRequest {
    SimulateRequest {
        enclosure: ENCLOSURE.to_string(),
        radiation: RADIATION.to_string(),
        space: SPACE.to_string(),
        listening_window: true,
        // Preference is the ranked score (README 11.1 / 2.6); CTA-2034 curves are required.
        cta2034: true,
        losses: true,
        walls: true,
        ..SimulateRequest::default()
    }
}

/// Scores a design as it stands (no correction added).
pub fn score_as_is(design: &DesignFile, registry: &Registry) -> (Score, SimulateResult) {
    let result = simulate(design, &request(), registry);
    let target = Target::of(design);
    let preference = result
        .cta2034
        .as_ref()
        .map(|report| report.score.preference);
    let shortfall = target.shortfall(preference, result.max_spl_metric_db);
    let over_budget = target
        .budget_max
        .is_some_and(|budget| result.cost.known_total_usd > budget);
    let problems: Vec<String> = result
        .issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .map(|issue| issue.message.clone())
        .collect();
    let score = Score {
        f6_hz: result.f6_hz,
        flatness_db: result.flatness_db,
        max_spl_db: result.max_spl_metric_db,
        cost_usd: result.cost.known_total_usd,
        cost_complete: result.cost.complete,
        cost_summary: result.cost.summary.clone(),
        meets_target: shortfall == 0.0 && !over_budget && problems.is_empty(),
        shortfall,
        interior_volume_m3: result.interior_volume_m3,
        problems,
        preference,
    };
    (score, result)
}

/// Corrects the design, then scores it (README section 11.1: metrics use the corrected response).
pub fn score(design: &DesignFile, registry: &Registry) -> (Score, DesignFile) {
    let corrected = apply_correction(design, registry);
    let (score, _) = score_as_is(&corrected, registry);
    (score, corrected)
}

/// One way of building, or one build to compare, with its score and its complete design.
#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    pub label: String,
    pub score: Score,
    pub design: DesignFile,
    pub notes: Vec<String>,
}

fn number(fields: &Map<String, Value>, key: &str) -> Option<f64> {
    fields.get(key).and_then(Value::as_f64)
}

/// The face sizes `[u, v]` of a box-like cabinet with outer size `[w, d, h]`.
fn face_size(surface: &str, size: [f64; 3]) -> Option<[f64; 2]> {
    match surface {
        "front" | "back" => Some([size[0], size[2]]),
        "left" | "right" => Some([size[1], size[2]]),
        "top" | "bottom" => Some([size[0], size[1]]),
        _ => None,
    }
}

/// A new outer shape for the same layout: `box` (printed), `sheet_box`, or `tube`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Printed { wall_m: f64 },
    Sheet { stock_id: String },
    Tube { stock_id: String },
}

impl Body {
    pub fn label(&self) -> String {
        match self {
            Body::Printed { .. } => "Printed box (PLA)".to_string(),
            Body::Sheet { stock_id } | Body::Tube { stock_id } => {
                let record = catalog::stock(stock_id).unwrap_or(Value::Null);
                let name = |key: &str| {
                    record
                        .get(key)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                let product = format!("{} {}", name("maker"), name("model"))
                    .trim()
                    .to_string();
                match self {
                    Body::Sheet { .. } => format!(
                        "Box cut from {}",
                        if product.is_empty() {
                            stock_id.clone()
                        } else {
                            product
                        }
                    ),
                    _ => format!(
                        "Tube of {} with printed end caps",
                        if product.is_empty() {
                            stock_id.clone()
                        } else {
                            product
                        }
                    ),
                }
            }
        }
    }
}

fn stock_number(stock_id: &str, keys: &[&str]) -> Option<f64> {
    let record = catalog::stock(stock_id)?;
    keys.iter()
        .find_map(|key| record.pointer(key).and_then(Value::as_f64))
}

/// Rebuilds the design's cabinet as `body`, keeping its proportions and its net air volume, and
/// moving mounts and straight ports with their faces. Returns the new design and notes.
pub fn rebuild(
    design: &DesignFile,
    body: &Body,
    registry: &Registry,
) -> Result<(DesignFile, Vec<String>), String> {
    let (s, _) = score_as_is(design, registry);
    let volume = s
        .interior_volume_m3
        .ok_or("the design has no air volume.")?;
    rebuild_to(design, body, volume, registry)
}

/// Inner height/width and depth/width for a free-proportion rebuild (README §11.3 Level 3).
/// Axes match [`place`]: width, depth, height.
pub type InnerProportions = (f64, f64);

impl Body {
    /// The way the design's cabinet is built now. Shapes other than a box, a sheet box, or a
    /// tube are rebuilt as a printed box.
    pub fn of(design: &DesignFile) -> Body {
        let outer = &design.speaker.enclosure.outer;
        let stock = outer
            .fields
            .get("stock_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        match (outer.kind.as_str(), stock) {
            ("sheet_box", Some(stock_id)) => Body::Sheet { stock_id },
            ("tube", Some(stock_id)) => Body::Tube { stock_id },
            ("box", _) => Body::Printed {
                wall_m: number(&outer.fields, "wall_m").unwrap_or(0.008),
            },
            _ => Body::Printed {
                wall_m: number(&outer.fields, "wall_m").unwrap_or(0.008),
            },
        }
    }
}

/// Like [`rebuild`], to a given net air volume, keeping the old cabinet's proportions.
pub fn rebuild_to(
    design: &DesignFile,
    body: &Body,
    target_volume: f64,
    registry: &Registry,
) -> Result<(DesignFile, Vec<String>), String> {
    rebuild_to_proportions(design, body, target_volume, None, registry)
}

/// Like [`rebuild_to`], optionally with free inner proportions `(tall, deep)` =
/// height/width and depth/width. Tubes ignore free proportions (pipe diameter is fixed).
pub fn rebuild_to_proportions(
    design: &DesignFile,
    body: &Body,
    target_volume: f64,
    proportions: Option<InnerProportions>,
    registry: &Registry,
) -> Result<(DesignFile, Vec<String>), String> {
    let enclosure = &design.speaker.enclosure;
    if !enclosure.chambers.is_empty() {
        return Err("a cabinet with several chambers is not rebuilt automatically yet.".into());
    }
    let outer = &enclosure.outer;
    let shape = registry
        .create_outer(&outer.kind, &outer.fields)
        .map_err(|err| err.to_string())?;
    let old_size = shape.outer_size_m()?;
    let mut notes = Vec::new();
    let old_wall = number(&outer.fields, "wall_m").unwrap_or(0.0);
    let mut inner = match (proportions, body) {
        (Some((tall, deep)), Body::Printed { .. } | Body::Sheet { .. })
            if tall.is_finite() && deep.is_finite() && tall > 0.1 && deep > 0.1 =>
        {
            let w = (target_volume / (tall * deep)).cbrt().max(0.01);
            [w, w * deep, w * tall]
        }
        _ => [
            (old_size[0] - 2.0 * old_wall).max(0.01),
            (old_size[1] - 2.0 * old_wall).max(0.01),
            (old_size[2] - 2.0 * old_wall).max(0.01),
        ],
    };
    let mut candidate = design.clone();
    for _ in 0..4 {
        candidate = place(design, body, inner, old_size)?;
        let (s, result) = score_as_is(&candidate, registry);
        if let Some(problem) = result
            .issues
            .iter()
            .find(|issue| issue.severity == Severity::Error)
        {
            return Err(problem.message.clone());
        }
        let volume = s
            .interior_volume_m3
            .filter(|v| *v > 0.0)
            .ok_or("the rebuilt cabinet has no air left inside.")?;
        let ratio = target_volume / volume;
        if (ratio - 1.0).abs() < 0.002 {
            break;
        }
        let scale = ratio.cbrt();
        match body {
            Body::Tube { .. } => inner[1] *= ratio,
            _ => inner.iter_mut().for_each(|side| *side *= scale),
        }
    }
    if let Body::Tube { .. } = body {
        notes.push(
            "A tube's diameter is set by the pipe; only its length matches the air volume.".into(),
        );
    } else if proportions.is_some() {
        notes.push("Cabinet proportions were free in the search (not locked to the start).".into());
    }
    Ok((candidate, notes))
}

fn place(
    design: &DesignFile,
    body: &Body,
    inner: [f64; 3],
    old_size: [f64; 3],
) -> Result<DesignFile, String> {
    let mut next = design.clone();
    let enclosure = &mut next.speaker.enclosure;
    let (new_size, fields): ([f64; 3], Map<String, Value>) = match body {
        Body::Printed { wall_m } => {
            let size = [
                inner[0] + 2.0 * wall_m,
                inner[1] + 2.0 * wall_m,
                inner[2] + 2.0 * wall_m,
            ];
            let mut fields = Map::new();
            fields.insert("width_m".into(), json!(size[0]));
            fields.insert("height_m".into(), json!(size[2]));
            fields.insert("depth_m".into(), json!(size[1]));
            fields.insert("wall_m".into(), json!(wall_m));
            fields.insert("edge_radius_m".into(), json!(0.0));
            enclosure.outer = KindPart {
                kind: "box".into(),
                fields,
            };
            (size, enclosure.outer.fields.clone())
        }
        Body::Sheet { stock_id } => {
            let t = stock_number(stock_id, &["/thickness_m", "/sizes/0/thickness_m"])
                .ok_or_else(|| format!("{stock_id} has no published thickness."))?;
            let size = [inner[0] + 2.0 * t, inner[1] + 2.0 * t, inner[2] + 2.0 * t];
            let mut fields = Map::new();
            fields.insert("stock_id".into(), json!(stock_id));
            fields.insert("width_m".into(), json!(size[0]));
            fields.insert("height_m".into(), json!(size[2]));
            fields.insert("depth_m".into(), json!(size[1]));
            enclosure.outer = KindPart {
                kind: "sheet_box".into(),
                fields,
            };
            (size, enclosure.outer.fields.clone())
        }
        Body::Tube { stock_id } => {
            if design.speaker.radiators.len() != 1 || !design.speaker.enclosure.ducts.is_empty() {
                return Err("a tube takes one driver on its end cap and no port, for now.".into());
            }
            let od = stock_number(
                stock_id,
                &["/outside_diameter_m", "/sizes/0/outside_diameter_m"],
            )
            .ok_or_else(|| format!("{stock_id} has no published outside diameter."))?;
            let cap = 0.008;
            let length = inner[1] + 2.0 * cap;
            let mut fields = Map::new();
            fields.insert("stock_id".into(), json!(stock_id));
            fields.insert("length_m".into(), json!(inner[1]));
            fields.insert("cap_thickness_m".into(), json!(cap));
            enclosure.outer = KindPart {
                kind: "tube".into(),
                fields,
            };
            ([od, length, od], enclosure.outer.fields.clone())
        }
    };
    let _ = fields;
    // Mounts keep their place on their face, in proportion.
    for mount in enclosure.mounts.iter_mut() {
        let (Some(old_face), Some(new_face)) = (
            face_size(&mount.surface, old_size),
            face_size(&mount.surface, new_size),
        ) else {
            return Err(format!(
                "mount {} is on a face this rebuild does not know.",
                mount.id
            ));
        };
        mount.at_m = [
            mount.at_m[0] / old_face[0] * new_face[0],
            mount.at_m[1] / old_face[1] * new_face[1],
        ];
        if let Body::Tube { .. } = body {
            mount.at_m = [new_size[0] / 2.0, new_size[2] / 2.0];
        }
    }
    // Straight ports keep their length from the face they leave, and their place in proportion.
    for duct in enclosure.ducts.iter_mut() {
        let Some(path) = duct.get("path_m").and_then(Value::as_array).cloned() else {
            continue;
        };
        let points: Vec<[f64; 3]> = path
            .iter()
            .filter_map(|p| {
                let a = p.as_array()?;
                Some([
                    a.first()?.as_f64()?,
                    a.get(1)?.as_f64()?,
                    a.get(2)?.as_f64()?,
                ])
            })
            .collect();
        let mut exit: Option<(usize, bool)> = None;
        for p in &points {
            for axis in 0..3 {
                if p[axis].abs() < 1e-6 {
                    exit = Some((axis, false));
                } else if (p[axis] - old_size[axis]).abs() < 1e-6 {
                    exit = Some((axis, true));
                }
            }
        }
        let moved: Vec<Value> = points
            .iter()
            .map(|p| {
                let mut q = [0.0; 3];
                for axis in 0..3 {
                    q[axis] = match exit {
                        Some((a, false)) if a == axis => p[axis],
                        Some((a, true)) if a == axis => new_size[axis] - (old_size[axis] - p[axis]),
                        _ => p[axis] / old_size[axis] * new_size[axis],
                    };
                }
                json!(q)
            })
            .collect();
        if let Some(object) = duct.as_object_mut() {
            object.insert("path_m".into(), Value::Array(moved));
        }
    }
    Ok(next)
}

/// Every way this cabinet can be built from the catalog (README section 6.2): printed, cut from
/// each sheet stock, and, for one driver without a port, a tube of each pipe it fits.
pub fn build_methods(
    design: &DesignFile,
    registry: &Registry,
) -> Vec<Result<Variant, (String, String)>> {
    let mut bodies = vec![Body::Printed {
        wall_m: number(&design.speaker.enclosure.outer.fields, "wall_m")
            .filter(|_| design.speaker.enclosure.outer.kind == "box")
            .unwrap_or(0.008),
    }];
    for record in catalog::all("catalog/stock/") {
        let Some(id) = record.get("id").and_then(Value::as_str) else {
            continue;
        };
        match record.get("kind").and_then(Value::as_str) {
            Some("sheet") => bodies.push(Body::Sheet {
                stock_id: id.to_string(),
            }),
            Some("pipe") | Some("tube") => bodies.push(Body::Tube {
                stock_id: id.to_string(),
            }),
            _ => {}
        }
    }
    bodies
        .iter()
        .map(|body| {
            let label = body.label();
            let (rebuilt, mut notes) =
                rebuild(design, body, registry).map_err(|err| (label.clone(), err))?;
            let (score, corrected) = score(&rebuilt, registry);
            if !score.problems.is_empty() {
                return Err((label, score.problems.join(" ")));
            }
            notes.push(
                "Wall vibration is not simulated, so stiff and floppy materials score alike."
                    .into(),
            );
            Ok(Variant {
                label,
                score,
                design: corrected,
                notes,
            })
        })
        .collect()
}

/// The reference driver closest to `id`: same kind, nearest cone area. A kind with no reference
/// part takes the nearest reference woofer or full-range unit.
fn reference_driver(id: &str) -> Option<(String, Option<String>)> {
    let driver = catalog::driver(id)?;
    let kind = driver.get("kind").and_then(Value::as_str).unwrap_or("");
    let sd = driver
        .pointer("/published/sd_m2")
        .and_then(Value::as_f64)
        .unwrap_or(0.01);
    let references: Vec<Value> = catalog::all("catalog/drivers/")
        .into_iter()
        .filter(|record| record.get("tier").and_then(Value::as_str) == Some("reference"))
        .collect();
    let pick = |want: &dyn Fn(&str) -> bool| {
        references
            .iter()
            .filter(|record| want(record.get("kind").and_then(Value::as_str).unwrap_or("")))
            .min_by(|a, b| {
                let distance = |r: &Value| {
                    let other = r
                        .pointer("/published/sd_m2")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.01);
                    (other / sd).ln().abs()
                };
                distance(a).total_cmp(&distance(b)).then_with(|| {
                    let price = |r: &Value| r.pointer("/price/amount").is_none();
                    price(a).cmp(&price(b))
                })
            })
            .and_then(|record| record.get("id").and_then(Value::as_str).map(str::to_string))
    };
    if let Some(found) = pick(&|k| k == kind) {
        return Some((found, None));
    }
    let found = pick(&|k| k == "woofer" || k == "fullrange" || k == "midrange")?;
    Some((
        found.clone(),
        Some(format!(
            "There is no reference {kind} in the catalog, so {found} stands in for {id}."
        )),
    ))
}

/// The design's reference build (README section 9.2): the closest reference drivers and
/// amplifier, in a box of the same net air volume cut from reference sheet stock.
pub fn reference_build(
    design: &DesignFile,
    registry: &Registry,
) -> Result<(DesignFile, Vec<String>), String> {
    let mut next = design.clone();
    let mut notes = Vec::new();
    for radiator in next.speaker.radiators.iter_mut() {
        let Some(id) = radiator
            .fields
            .get("driver_id")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let (swap, note) =
            reference_driver(&id).ok_or_else(|| format!("no reference driver for {id}."))?;
        if let Some(note) = note {
            notes.push(note);
        }
        radiator.fields.insert("driver_id".into(), json!(swap));
    }
    let amplifier = catalog::all("catalog/electronics/")
        .into_iter()
        .find(|record| {
            record.get("tier").and_then(Value::as_str) == Some("reference")
                && record.get("kind").and_then(Value::as_str) == Some("amplifier")
        })
        .and_then(|record| record.get("id").and_then(Value::as_str).map(str::to_string))
        .ok_or("there is no reference amplifier in the catalog.")?;
    next.electronics.amplifiers = vec![amplifier];
    next.electronics.dsp_device = None;
    // Corrections were made for the cheap drivers; the reference build gets its own.
    for chain in next.dsp.per_radiator.iter_mut() {
        chain
            .blocks
            .retain(|block| block.fields.get("correction") != Some(&Value::Bool(true)));
    }
    let sheet = catalog::all("catalog/stock/")
        .into_iter()
        .filter(|record| {
            record.get("tier").and_then(Value::as_str) == Some("reference")
                && record.get("kind").and_then(Value::as_str) == Some("sheet")
        })
        .filter_map(|record| {
            let id = record.get("id").and_then(Value::as_str)?.to_string();
            let price = record.pointer("/price/amount").and_then(Value::as_f64);
            Some((id, price))
        })
        .min_by(|a, b| a.1.unwrap_or(f64::MAX).total_cmp(&b.1.unwrap_or(f64::MAX)))
        .ok_or("there is no reference sheet stock in the catalog.")?;
    // The volume to keep is the cheap design's, with its own drivers.
    let with_volume = |mut d: DesignFile| {
        d.speaker.radiators = design.speaker.radiators.clone();
        d
    };
    let (mut rebuilt, more) = rebuild(
        &with_volume(next.clone()),
        &Body::Sheet { stock_id: sheet.0 },
        registry,
    )?;
    rebuilt.speaker.radiators = next.speaker.radiators.clone();
    notes.extend(more);
    notes.push(
        "Same layout and the same net air volume; the reference drivers may want a different box."
            .into(),
    );
    Ok((rebuilt, notes))
}

/// The cheap design and its reference build, each corrected and scored.
#[derive(Debug, Clone, Serialize)]
pub struct CheapAgainstReference {
    pub cheap: Variant,
    pub reference: Option<Variant>,
    pub reference_error: Option<String>,
}

pub fn cheap_against_reference(design: &DesignFile, registry: &Registry) -> CheapAgainstReference {
    let (cheap_score, cheap_design) = score(design, registry);
    let cheap = Variant {
        label: "This design".into(),
        score: cheap_score,
        design: cheap_design,
        notes: Vec::new(),
    };
    match reference_build(design, registry) {
        Ok((reference, notes)) => {
            let (score, corrected) = score(&reference, registry);
            CheapAgainstReference {
                cheap,
                reference: Some(Variant {
                    label: "Reference build".into(),
                    score,
                    design: corrected,
                    notes,
                }),
                reference_error: None,
            }
        }
        Err(error) => CheapAgainstReference {
            cheap,
            reference: None,
            reference_error: Some(error),
        },
    }
}
