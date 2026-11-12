//! The generator (README section 11.4): whole cabinets the lab proposes.
//!
//! A candidate is a complete design: an outer form and its proportions, an inside layout, a
//! driver from the catalog, the face it sits on, and where on that face. Candidates are
//! grouped into families (one outer form, one inside layout, one facing); each family is
//! searched with a seeded random search and then by refining its best, every candidate
//! corrected and scored like any design. What the builder cannot make or print is thrown out.

use seatlock_core::catalog;
use seatlock_core::constants::C0;
use seatlock_core::design::DesignFile;
use seatlock_core::registry::Registry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::compare::{score, Score};
use crate::optimize::{objective, port_length_m, Point, SplitMix};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerateRequest {
    pub seed: u64,
    pub family: usize,
    /// Candidates scored in this family.
    pub evaluations: usize,
}

impl Default for GenerateRequest {
    fn default() -> Self {
        GenerateRequest {
            seed: 1,
            family: 0,
            evaluations: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Family {
    pub index: usize,
    pub label: String,
    /// The outer form's kind.
    pub outer: String,
    /// `sealed`, `vented`, or `line`.
    pub layout: String,
    /// The face the driver sits on.
    pub face: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Generated {
    pub family: usize,
    pub summary: String,
    pub score: Score,
    pub design: DesignFile,
    pub variables: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FamilyResult {
    pub family: Family,
    pub points: Vec<Point>,
    /// The best few that differ, best first.
    pub best: Vec<Generated>,
    pub evaluations: usize,
    /// Candidates the builder could not make or print, which were not scored.
    pub thrown_out: usize,
    pub canceled: bool,
    pub notes: Vec<String>,
}

const OUTERS: [(&str, &str); 5] = [
    ("box", "Box"),
    ("rounded", "Rounded box"),
    ("chamfered", "Chamfered box"),
    ("sloped", "Leaning front"),
    ("prism", "Tapered front"),
];

/// The families the generator searches.
pub fn families() -> Vec<Family> {
    let mut out: Vec<Family> = Vec::new();
    let mut push = |label: String, outer: &str, layout: &str, face: &str| {
        out.push(Family {
            index: out.len(),
            label,
            outer: outer.into(),
            layout: layout.into(),
            face: face.into(),
        });
    };
    for (kind, name) in OUTERS {
        push(format!("{name}, sealed"), kind, "sealed", "front");
        push(format!("{name}, ported"), kind, "vented", "front");
    }
    push("Box, folded line".into(), "box", "line", "front");
    for (face, name) in [("top", "driver facing up"), ("back", "driver facing back")] {
        push(format!("Box, sealed, {name}"), "box", "sealed", face);
        push(format!("Box, ported, {name}"), "box", "vented", face);
    }
    out
}

/// A catalog driver the generator may build with.
#[derive(Debug, Clone)]
struct Part {
    id: String,
    name: String,
    fs_hz: f64,
    sd_m2: f64,
    outer_m: f64,
    depth_m: f64,
}

/// Sourced budget drivers that can carry a cabinet's bass and whose record prints the sizes a
/// cut-out needs (README 9.1, 11.4).
fn parts() -> Vec<Part> {
    catalog::all("catalog/drivers/")
        .into_iter()
        .filter_map(|record| {
            let text = |key: &str| record.get(key).and_then(Value::as_str);
            if text("status") != Some("sourced") || text("tier") != Some("budget") {
                return None;
            }
            if !matches!(text("kind"), Some("woofer" | "fullrange" | "midrange")) {
                return None;
            }
            let number = |path: &str| record.pointer(path).and_then(Value::as_f64);
            number("/mounting/cutout_diameter_m").filter(|v| *v > 0.0)?;
            Some(Part {
                id: text("id")?.to_string(),
                name: format!("{} {}", text("maker")?, text("model")?),
                fs_hz: number("/published/fs_hz")?,
                sd_m2: number("/published/sd_m2")?,
                outer_m: number("/mounting/outer_diameter_m").filter(|v| *v > 0.0)?,
                depth_m: number("/mounting/depth_m").unwrap_or(0.06),
            })
        })
        .collect()
}

const WALL_M: f64 = 0.008;
/// The room a candidate is judged in when its driver does not face the listener and the
/// starting design has no room of its own: the example of README section 5.10.
fn default_room() -> Value {
    json!({ "kind": "simple", "wall_behind_m": 0.5, "wall_reflection": 0.9, "volume_m3": 50.0,
            "rt60_s": 0.4 })
}

fn lerp(low: f64, high: f64, x: f64) -> f64 {
    low + (high - low) * x.clamp(0.0, 1.0)
}

/// The outer shape of `kind` around inner air about `w` wide, `h` high and `d` deep, shaped by
/// `t` in `[0, 1]`. Returns the fields and the outer width, height, and depth at the front's
/// middle.
fn outer_fields(kind: &str, w: f64, h: f64, d: f64, t: f64) -> (Value, [f64; 3]) {
    let (ow, oh, od) = (w + 2.0 * WALL_M, h + 2.0 * WALL_M, d + 2.0 * WALL_M);
    let fields = match kind {
        "rounded" => json!({ "kind": kind, "width_m": ow, "height_m": oh, "depth_m": od,
            "wall_m": WALL_M, "edge_radius_m": WALL_M + lerp(0.004, 0.2 * ow.min(od), t) }),
        "chamfered" => json!({ "kind": kind, "width_m": ow, "height_m": oh, "depth_m": od,
            "wall_m": WALL_M, "chamfer_m": WALL_M + lerp(0.004, 0.2 * ow.min(od), t) }),
        "sloped" => {
            let lean = lerp(0.1, 0.4, t) * od;
            json!({ "kind": kind, "width_m": ow, "height_m": oh,
                "depth_bottom_m": od + lean / 2.0, "depth_top_m": od - lean / 2.0,
                "wall_m": WALL_M })
        }
        "prism" => {
            // A front that narrows toward the top.
            let inset = lerp(0.05, 0.22, t) * ow;
            json!({ "kind": kind, "outline_m": [[ow, 0.0], [ow - inset, oh], [inset, oh], [0.0, 0.0]],
                "length_m": od, "wall_m": WALL_M })
        }
        _ => json!({ "kind": "box", "width_m": ow, "height_m": oh, "depth_m": od,
            "wall_m": WALL_M, "edge_radius_m": 0.0 }),
    };
    (fields, [ow, oh, od])
}

struct Built {
    design: DesignFile,
    named: Vec<(String, f64)>,
    summary: String,
}

/// The candidate at `x` (every entry in `[0, 1]`), or why it cannot be built.
fn build(
    base: &DesignFile,
    family: &Family,
    parts: &[Part],
    x: &[f64; 9],
    registry: &Registry,
) -> Result<Built, String> {
    let part = &parts[((x[0] * parts.len() as f64) as usize).min(parts.len() - 1)];
    let cone = (4.0 * part.sd_m2 / std::f64::consts::PI).sqrt();
    // Net air from 1.5 to 40 L on a log scale; a line's chamber is smaller.
    let volume = 0.0015 * (40.0f64 / 1.5).powf(x[1]);
    let tall = lerp(1.0, 2.4, x[2]);
    let deep = lerp(0.7, 1.6, x[3]);
    let along = lerp(0.3, 0.72, x[4]);
    let shape_t = x[6];
    let line = family.layout == "line";

    // Inner air w x h x d of the asked volume and proportions, then the true shape scaled
    // until its own air matches.
    let mut w = (volume / (tall * deep)).cbrt();
    let mut fields = Value::Null;
    let mut size = [0.0; 3];
    for _ in 0..4 {
        let (h, d) = (w * tall, w * deep);
        (fields, size) = outer_fields(&family.outer, w, h, d, shape_t);
        let kind = fields["kind"].as_str().unwrap_or("box").to_string();
        let map = fields.as_object().cloned().unwrap_or_default();
        let air = registry.create_outer(&kind, &map)?.interior_volume_m3()?;
        if air <= 0.0 {
            return Err("the shape leaves no air.".into());
        }
        w *= (volume / air).cbrt();
    }
    let [ow, oh, od] = size;
    if ow < part.outer_m + 2.0 * WALL_M + 0.012 {
        return Err("the cabinet is narrower than the driver.".into());
    }
    let (face_w, face_h) = match family.face.as_str() {
        "top" => (ow, od),
        _ => (ow, oh),
    };
    if face_h < part.outer_m + 2.0 * WALL_M + 0.012 {
        return Err("the face is shorter than the driver.".into());
    }
    let reach = part.outer_m / 2.0 + WALL_M + 0.006;
    let v = (along * face_h).clamp(reach, face_h - reach);
    let facing_depth = if family.face == "top" { oh } else { od };
    if facing_depth < part.depth_m + 2.0 * WALL_M + 0.01 {
        return Err("the cabinet is too shallow for the driver.".into());
    }
    let mut named = vec![
        ("air L".to_string(), volume * 1000.0),
        ("width mm".to_string(), ow * 1000.0),
        ("height mm".to_string(), oh * 1000.0),
        ("depth mm".to_string(), od * 1000.0),
        ("driver along its face %".to_string(), v / face_h * 100.0),
    ];
    let mut summary = format!(
        "{}, {:.1} L, {:.0} x {:.0} x {:.0} mm",
        part.name,
        volume * 1000.0,
        ow * 1000.0,
        oh * 1000.0,
        od * 1000.0
    );

    let mut chambers: Vec<Value> = Vec::new();
    let mut ducts: Vec<Value> = Vec::new();
    match family.layout.as_str() {
        "vented" => {
            let tuning = part.fs_hz * lerp(0.65, 1.35, x[5]);
            let diameter = (0.35 * cone).clamp(0.025, 0.08);
            let length = port_length_m(diameter, tuning, volume);
            // The port leaves the front, or the back when the front leans.
            let back = family.outer == "sloped";
            let port_depth = if back { od * 0.75 } else { od };
            if length < 0.01 || length > port_depth - 0.05 {
                return Err("no port of this tuning fits the cabinet.".into());
            }
            // At the end of the height away from a front driver.
            let low = WALL_M + diameter / 2.0 + 0.012;
            let z = if family.face == "front" && v < oh / 2.0 {
                oh - low
            } else {
                low
            };
            let px = ow / 2.0;
            let path = if back {
                let y = fields["depth_bottom_m"].as_f64().unwrap_or(od);
                json!([[px, y - length, low], [px, y, low]])
            } else {
                json!([[px, length, z], [px, 0.0, z]])
            };
            ducts.push(
                json!({ "id": "port", "from": "main", "to": "outside", "path_m": path,
                "section": { "shape": "round", "start_m": [diameter], "end_m": [diameter] } }),
            );
            named.push(("tuning Hz".into(), tuning));
            summary.push_str(&format!(", port tuned to {tuning:.0} Hz"));
        }
        "line" => {
            // A chamber behind the driver at the top; below it a slot that runs down the
            // back, forward along the bottom, up behind the front, and out through it.
            let chamber_h = (part.outer_m + 0.03).max(0.3 * oh).min(oh - 0.25);
            let floor = oh - WALL_M - chamber_h;
            let slot_w = (ow - 2.0 * WALL_M - 0.02).min(0.9 * ow);
            let start = lerp(0.05, 0.1, x[5]).min((od - 3.0 * WALL_M) / 2.0 - 0.012);
            let end = start * lerp(0.6, 1.0, x[7]);
            let y_back = od - WALL_M - start / 2.0 - 0.006;
            let y_front = WALL_M + start / 2.0 + 0.006;
            let z_low = WALL_M + start / 2.0 + 0.006;
            let z_out = lerp(z_low + start + 0.03, floor - start / 2.0 - 0.02, x[8]);
            if floor < z_low + 2.0 * start + 0.05 || y_back - y_front < start + 0.012 {
                return Err("the cabinet is too small for a folded line.".into());
            }
            let top = oh - WALL_M;
            chambers.push(json!({ "id": "main", "min_m": [WALL_M, WALL_M, floor],
                "max_m": [ow - WALL_M, od - WALL_M, top], "fill": null }));
            let px = ow / 2.0;
            ducts.push(json!({ "id": "line", "from": "main", "to": "outside",
                "path_m": [[px, y_back, floor + 0.05], [px, y_back, z_low], [px, y_front, z_low],
                           [px, y_front, z_out], [px, 0.0, z_out]],
                "section": { "shape": "rect", "start_m": [slot_w, start], "end_m": [slot_w, end] } }));
            let length = (floor + 0.05 - z_low) + (y_back - y_front) + (z_out - z_low) + y_front;
            named.push(("line m".into(), length));
            named.push(("line start mm".into(), start * 1000.0));
            named.push(("line end mm".into(), end * 1000.0));
            summary.push_str(&format!(
                ", {length:.2} m folded line (quarter wave near {:.0} Hz)",
                C0 / (4.0 * length)
            ));
        }
        _ => {}
    }
    // The driver: on a line cabinet it sits in the chamber at the top.
    let at = if line {
        let chamber_h = (part.outer_m + 0.03).max(0.3 * oh).min(oh - 0.25);
        [face_w / 2.0, oh - WALL_M - chamber_h / 2.0]
    } else {
        [face_w / 2.0, v]
    };
    let facing = family.face != "front";
    if facing {
        summary.push_str(match family.face.as_str() {
            "top" => ", facing up",
            _ => ", facing back",
        });
    }
    let room = match (&base.speaker.room, facing) {
        (Some(room), _) => serde_json::to_value(room).unwrap_or(Value::Null),
        (None, true) => default_room(),
        (None, false) => Value::Null,
    };
    let mut speaker = json!({
        "enclosure": { "outer": fields, "chambers": chambers, "ducts": ducts,
            "mounts": [{ "id": "m1", "surface": family.face, "at_m": at, "rear_into": "main",
                         "recess_m": 0.0 }],
            "solids": [] },
        "radiators": [{ "kind": "cone", "driver_id": part.id, "mount": "m1" }],
        "listening": base.speaker.listening,
    });
    if !room.is_null() {
        speaker["room"] = room;
    }
    let design: DesignFile = serde_json::from_value(json!({
        "schema_version": base.schema_version,
        "name": format!("generated-{}", family.label.to_lowercase().replace([',', ' '], "-").replace("--", "-")),
        "speaker": speaker,
        "dsp": {
            "sample_rate_hz": base.dsp.sample_rate_hz,
            "whole_speaker": [],
            "per_radiator": [{ "blocks": [], "delay_s": 0.0, "gain_db": 0.0, "invert": false,
                               "limiter": null }],
            "correction": base.dsp.correction,
        },
        "electronics": base.electronics,
        "build": base.build,
        "target": base.target,
    }))
    .map_err(|error| error.to_string())?;
    // The builder must be able to make it and print it.
    let report = crate::geometry_report_with(&design, false)?;
    if !report.can_export_print {
        return Err(report
            .warnings
            .last()
            .cloned()
            .unwrap_or_else(|| "the cabinet cannot be printed.".into()));
    }
    Ok(Built {
        design,
        named,
        summary,
    })
}

/// Searches one family. `progress(done, total)` is called after every candidate and stops the
/// search when it returns false.
pub fn generate_family(
    base: &DesignFile,
    request: &GenerateRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<FamilyResult, String> {
    let family = families()
        .get(request.family)
        .cloned()
        .ok_or_else(|| format!("there is no family {}.", request.family))?;
    let parts = parts();
    if parts.is_empty() {
        return Err("the catalog has no sourced budget driver with mounting sizes.".into());
    }
    let mut notes = vec![format!(
        "Drivers tried: {}. Only sourced budget drivers whose records print their mounting sizes \
         are proposed; one driver per cabinet for now.",
        parts
            .iter()
            .map(|part| part.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    )];
    if family.face != "front" {
        notes.push(
            "The driver does not face the listener, so these are judged in a room. The everyday \
             model is rough for such a driver: run the outside in 3D on a result before \
             trusting it."
                .into(),
        );
    }
    let mut random =
        SplitMix(request.seed ^ (request.family as u64 + 1).wrapping_mul(0x5851_F42D_4C95_7F2D));
    let budget = request.evaluations.max(4);
    let explore = (budget * 3).div_ceil(5);
    let mut points = Vec::new();
    let mut best: Vec<(f64, [f64; 9], Generated)> = Vec::new();
    let mut scored = 0usize;
    let mut thrown_out = 0usize;
    let mut reasons: Vec<(String, usize)> = Vec::new();
    let mut canceled = false;
    // Building is cheap and scoring is not, so a candidate that cannot be built costs a try,
    // not an evaluation.
    let mut tries = 0usize;
    while scored < budget && tries < budget * 40 {
        tries += 1;
        let x: [f64; 9] = if scored < explore || best.is_empty() {
            std::array::from_fn(|_| random.next())
        } else {
            // Refine one of the best: small moves, and now and then another driver.
            let from = &best[(random.next() * best.len().min(2) as f64) as usize].1;
            let spread = lerp(
                0.18,
                0.05,
                (scored - explore) as f64 / (budget - explore) as f64,
            );
            std::array::from_fn(|index| {
                if index == 0 {
                    if random.next() < 0.2 {
                        random.next()
                    } else {
                        from[0]
                    }
                } else {
                    (from[index] + spread * (2.0 * random.next() - 1.0)).clamp(0.0, 1.0)
                }
            })
        };
        let built = match build(base, &family, &parts, &x, registry) {
            Ok(built) => built,
            Err(reason) => {
                thrown_out += 1;
                match reasons.iter_mut().find(|(text, _)| *text == reason) {
                    Some(entry) => entry.1 += 1,
                    None => reasons.push((reason, 1)),
                }
                continue;
            }
        };
        let (score, corrected) = score(&built.design, registry);
        scored += 1;
        let value = objective(&score);
        points.push(Point {
            cost_usd: score.cost_usd,
            shortfall: score.shortfall,
            meets_target: score.meets_target,
        });
        let candidate = Generated {
            family: family.index,
            summary: format!("{}: {}", family.label, built.summary),
            score,
            design: corrected,
            variables: built.named,
        };
        // Keep the best few that differ in driver or by more than a few percent in size.
        let close = |other: &Generated| {
            other.design.speaker.radiators == candidate.design.speaker.radiators
                && other
                    .variables
                    .iter()
                    .zip(&candidate.variables)
                    .all(|((_, a), (_, b))| (a - b).abs() <= 0.06 * a.abs().max(b.abs()).max(1.0))
        };
        let keep = match best.iter().position(|(_, _, other)| close(other)) {
            Some(index) if best[index].0 <= value => false,
            Some(index) => {
                best.remove(index);
                true
            }
            None => true,
        };
        if keep {
            best.push((value, x, candidate));
            best.sort_by(|a, b| a.0.total_cmp(&b.0));
            best.truncate(4);
        }
        if !progress(scored, budget) {
            canceled = true;
            break;
        }
    }
    reasons.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    if let Some((reason, count)) = reasons.first() {
        notes.push(format!(
            "{thrown_out} proposals could not be built and were not scored; most often ({count}): {reason}"
        ));
    }
    if scored == 0 {
        return Err(format!(
            "no proposal in this family could be built: {}",
            reasons
                .first()
                .map(|(reason, _)| reason.as_str())
                .unwrap_or("no reason recorded")
        ));
    }
    Ok(FamilyResult {
        family,
        points,
        best: best
            .into_iter()
            .map(|(_, _, candidate)| candidate)
            .collect(),
        evaluations: scored,
        thrown_out,
        canceled,
        notes,
    })
}
