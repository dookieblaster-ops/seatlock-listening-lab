//! The generator (README section 11.4): whole cabinets the lab proposes.
//!
//! A candidate is a complete design: an outer form and its proportions, an inside layout, a
//! driver from the catalog, the face it sits on, and where on that face. Candidates are
//! grouped into families (one outer form, one inside layout, one facing); each family is
//! searched with a seeded random search interleaved with refining its best, every candidate
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
    /// Frequency bands: 1 is one driver, 2 adds a tweeter on the front with a crossover, 3
    /// adds a midrange between them in its own sealed chamber.
    #[serde(default = "one")]
    pub ways: u32,
    /// Bass drivers: 2 puts a second one above the tweeter, in the same chamber and on the
    /// same channel settings (a woofer, tweeter, woofer column).
    #[serde(default = "one")]
    pub woofers: u32,
}

fn one() -> u32 {
    1
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

const OUTERS: [(&str, &str); 6] = [
    ("box", "Box"),
    ("rounded", "Rounded box"),
    ("chamfered", "Chamfered box"),
    ("sloped", "Leaning front"),
    ("prism", "Tapered front"),
    ("revolved", "Egg"),
];

/// The families the generator searches.
pub fn families() -> Vec<Family> {
    let mut out: Vec<Family> = Vec::new();
    let mut push_full =
        |label: String, outer: &str, layout: &str, face: &str, ways: u32, woofers: u32| {
            out.push(Family {
                index: out.len(),
                label,
                outer: outer.into(),
                layout: layout.into(),
                face: face.into(),
                ways,
                woofers,
            });
        };
    let mut push_ways = |label: String, outer: &str, layout: &str, face: &str, ways: u32| {
        push_full(label, outer, layout, face, ways, 1)
    };
    let mut push = |label: String, outer: &str, layout: &str, face: &str| {
        push_ways(label, outer, layout, face, 1)
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
    // Two-ways: a woofer and a tweeter on the front, split by a digital crossover. Added
    // after the one-driver families so those keep their numbers.
    for (label, outer, layout, face) in [
        ("Box, sealed, woofer and tweeter", "box", "sealed", "front"),
        ("Box, ported, woofer and tweeter", "box", "vented", "front"),
        (
            "Rounded box, ported, woofer and tweeter",
            "rounded",
            "vented",
            "front",
        ),
        (
            "Leaning front, ported, woofer and tweeter",
            "sloped",
            "vented",
            "front",
        ),
        (
            "Box, folded line, woofer and tweeter",
            "box",
            "line",
            "front",
        ),
        (
            "Box, ported, woofer facing up, tweeter in front",
            "box",
            "vented",
            "top",
        ),
        (
            "Box, ported, woofer facing back, tweeter in front",
            "box",
            "vented",
            "back",
        ),
    ] {
        push_ways(label.into(), outer, layout, face, 2);
    }
    // More sound sources (README 2.6, "any configuration"): two woofers around a tweeter,
    // and three-ways with a midrange in its own sealed chamber. Added last, so earlier
    // families keep their numbers.
    for (label, layout, ways, woofers) in [
        ("Box, sealed, two woofers around a tweeter", "sealed", 2, 2),
        ("Box, ported, two woofers around a tweeter", "vented", 2, 2),
        ("Box, sealed, three-way", "sealed", 3, 1),
        ("Box, ported, three-way", "vented", 3, 1),
    ] {
        push_full(label.into(), "box", layout, "front", ways, woofers);
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
    /// The lowest cone break-up an independent test found (README 9.1).
    breakup_hz: Option<f64>,
}

/// Sourced budget drivers that can carry a cabinet's bass and whose record prints the sizes a
/// cut-out needs (README 9.1, 11.4).
fn parts() -> Vec<Part> {
    parts_of(&["woofer", "fullrange", "midrange"])
}

/// Sourced budget tweeters whose records give the sizes a cut-out needs.
fn tweeters() -> Vec<Part> {
    parts_of(&["tweeter"])
}

fn parts_of(kinds: &[&str]) -> Vec<Part> {
    catalog::all("catalog/drivers/")
        .into_iter()
        .filter_map(|record| {
            let text = |key: &str| record.get(key).and_then(Value::as_str);
            if text("status") != Some("sourced") || text("tier") != Some("budget") {
                return None;
            }
            if !kinds.contains(&text("kind")?) {
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
                breakup_hz: seatlock_acoustics::breakup(&record).map(|(f, _)| f),
            })
        })
        .collect()
}

const WALL_M: f64 = 0.008;
/// Search variables per candidate: driver, volume, two proportions, driver place, tuning or
/// line size, shape, line taper, line exit, then tweeter, crossover frequency, tweeter level,
/// then midrange, the lower crossover frequency, and the midrange's level.
const VARIABLES: usize = 15;
/// The search variables that pick a part from a list: moved by jumps, not small steps.
const CHOICES: [usize; 3] = [0, 9, 12];
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
    if kind == "revolved" {
        // As wide as it is high, widest two fifths of the way back, with a flat front for
        // the driver and a smaller flat back.
        let across = (w * h).sqrt() + 2.0 * WALL_M;
        let fields = json!({ "kind": kind, "diameter_m": across, "length_m": od,
            "front_diameter_m": lerp(0.55, 0.85, t) * across,
            "back_diameter_m": 0.4 * across, "widest_at": 0.4, "wall_m": WALL_M });
        return (fields, [across, across, od]);
    }
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
    tweeters: &[Part],
    mids: &[Part],
    x: &[f64; VARIABLES],
    registry: &Registry,
) -> Result<Built, String> {
    let part = &parts[((x[0] * parts.len() as f64) as usize).min(parts.len() - 1)];
    let cone = (4.0 * part.sd_m2 / std::f64::consts::PI).sqrt();
    let deep = lerp(0.7, 1.6, x[3]);
    let shape_t = x[6];
    let line = family.layout == "line";
    let tweeter = match family.ways {
        1 => None,
        _ => Some(
            tweeters
                .get(((x[9] * tweeters.len() as f64) as usize).min(tweeters.len().max(1) - 1))
                .ok_or("the catalog has no sourced budget tweeter with mounting sizes.")?,
        ),
    };
    let mid = match family.ways {
        3 => Some(
            mids.get(((x[12] * mids.len() as f64) as usize).min(mids.len().max(1) - 1))
                .ok_or("the catalog has no sourced budget midrange with mounting sizes.")?,
        ),
        _ => None,
    };
    if mid.is_some_and(|m| m.id == part.id) {
        return Err("the midrange would be the woofer itself.".into());
    }
    // Room on the front for the tweeter above (or apart from) the bass driver, a midrange
    // between them, and a second woofer above the tweeter.
    let gap = 0.012;
    let tweeter_room = tweeter.map_or(0.0, |t| t.outer_m + gap);
    let mid_room = mid.map_or(0.0, |m| m.outer_m + gap);
    let second_room = if family.woofers > 1 {
        part.outer_m + gap
    } else {
        0.0
    };
    let stacked_front = tweeter_room + mid_room + second_room;

    // Multi-source harden (README 2.6): stacked faces and folded lines need taller, larger
    // cabinets than a single driver. Raise the tall/volume floors so random draws land in
    // buildable sizes more often (the 8 in budget woofers made short throws common).
    let mut tall = lerp(1.0, 2.4, x[2]);
    let mut volume = 0.0015 * (40.0f64 / 1.5).powf(x[1]);
    let tall_floor = if family.woofers > 1 {
        2.0
    } else if line && tweeter.is_some() {
        2.2
    } else if mid.is_some() {
        1.9
    } else if tweeter.is_some() {
        1.55
    } else if line {
        1.7
    } else {
        1.0
    };
    tall = tall.max(tall_floor);
    let volume_floor = if family.woofers > 1 {
        0.012
    } else if line && tweeter.is_some() {
        0.01
    } else if mid.is_some() {
        0.008
    } else if line {
        0.006
    } else {
        0.0015
    };
    volume = volume.max(volume_floor).min(0.06);

    // Inner air w x h x d of the asked volume and proportions, then the true shape scaled
    // until its own air matches. Grow tall/volume a few times if the face still cannot hold
    // the stack (large woofers on multi-source families).
    let mut fields = Value::Null;
    let mut size = [0.0; 3];
    let front = family.face == "front";
    let stack_on_face = if front { stacked_front } else { 0.0 };
    for _grow in 0..6 {
        let mut w = (volume / (tall * deep)).cbrt();
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
        let [_ow, oh, od] = size;
        let face_h = match family.face.as_str() {
            "top" => od,
            _ => oh,
        };
        let need =
            part.outer_m + stack_on_face + 2.0 * WALL_M + 0.012 + if line { 0.22 } else { 0.0 };
        if face_h + 1e-9 >= need {
            break;
        }
        tall = (tall * 1.18).min(3.6);
        volume = (volume * 1.12).min(0.06);
    }
    let [ow, oh, od] = size;
    if ow < part.outer_m + 2.0 * WALL_M + 0.012 {
        return Err("the cabinet is narrower than the driver.".into());
    }
    if let Some(t) = tweeter {
        if ow < t.outer_m + 2.0 * WALL_M + 0.012 {
            return Err("the cabinet is narrower than the tweeter.".into());
        }
        if od < t.depth_m + 2.0 * WALL_M + 0.01 {
            return Err("the cabinet is too shallow for the tweeter.".into());
        }
    }
    if let Some(m) = mid {
        if ow < m.outer_m + 2.0 * WALL_M + 0.012 {
            return Err("the cabinet is narrower than the midrange.".into());
        }
        if od < m.depth_m + 2.0 * WALL_M + 0.02 {
            return Err("the cabinet is too shallow for the midrange.".into());
        }
    }
    // Both on the front: the tweeter sits right above the bass driver.
    let stacked = stack_on_face;
    let (face_w, face_h) = match family.face.as_str() {
        "top" => (ow, od),
        _ => (ow, oh),
    };
    if face_h < part.outer_m + stacked + 2.0 * WALL_M + 0.012 {
        return Err("the face is shorter than its drivers.".into());
    }
    if tweeter.is_some() && !front && oh < tweeter_room + 2.0 * WALL_M + 0.012 {
        return Err("the front is shorter than the tweeter.".into());
    }
    // Keep a front woofer higher when a port and a tweeter share the baffle, so the port
    // still fits under the cone.
    let along = if front && tweeter.is_some() && family.layout == "vented" {
        lerp(0.48, 0.72, x[4])
    } else {
        lerp(0.3, 0.72, x[4])
    };
    let reach = part.outer_m / 2.0 + WALL_M + 0.006;
    let egg = family.outer == "revolved";
    if egg && fields["front_diameter_m"].as_f64().unwrap_or(0.0) < part.outer_m + 0.012 {
        return Err("the flat front is smaller than the driver.".into());
    }
    // On an egg the driver sits in the middle of the flat front.
    let mut v = if egg {
        face_h / 2.0
    } else {
        (along * face_h).clamp(reach, face_h - reach - stacked)
    };
    // Front port at the bottom needs clearance under the woofer when a tweeter is stacked.
    if front && tweeter.is_some() && family.layout == "vented" && family.outer != "sloped" && !egg {
        let port_clear = WALL_M + 0.08 / 2.0 + 0.012 + 0.01 + part.outer_m / 2.0;
        let max_v = face_h - reach - stacked;
        if v < port_clear && port_clear <= max_v {
            v = port_clear;
        }
    }
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
    // A three-way: a partition just above the woofer; above it the midrange's own sealed
    // chamber, which the tweeter (sealed at the back) also sits in front of.
    let mut bass_volume = volume;
    let mut mid_z = 0.0;
    if let Some(m) = mid {
        let partition_low = v + part.outer_m / 2.0 + 0.002;
        let split = partition_low + WALL_M;
        mid_z = split + 0.002 + m.outer_m / 2.0;
        let across = (ow - 2.0 * WALL_M) * (od - 2.0 * WALL_M);
        bass_volume = across * (partition_low - WALL_M);
        let mid_volume = across * (oh - WALL_M - split);
        if bass_volume < 0.002 || mid_volume < 0.0003 {
            return Err("the chambers for the woofer and the midrange are too small.".into());
        }
        chambers.push(json!({ "id": "main", "min_m": [WALL_M, WALL_M, WALL_M],
            "max_m": [ow - WALL_M, od - WALL_M, partition_low], "fill": null }));
        chambers.push(json!({ "id": "mid", "min_m": [WALL_M, WALL_M, split],
            "max_m": [ow - WALL_M, od - WALL_M, oh - WALL_M], "fill": null }));
        named.push(("woofer air L".into(), bass_volume * 1000.0));
        named.push(("midrange air L".into(), mid_volume * 1000.0));
    }
    match family.layout.as_str() {
        "vented" => {
            let mut tuning = part.fs_hz * lerp(0.65, 1.35, x[5]);
            let mut diameter = (0.35 * cone).clamp(0.025, 0.08);
            // The port leaves the front, or the back when the front leans.
            let back = family.outer == "sloped" || egg;
            if egg && fields["back_diameter_m"].as_f64().unwrap_or(0.0) < diameter + 0.02 {
                return Err("the flat back is smaller than the port.".into());
            }
            let port_depth = if back { od * 0.75 } else { od };
            let max_length = port_depth - 0.05;
            let mut length = port_length_m(diameter, tuning, bass_volume);
            // Harden: widen the port, then raise the tuning, until a physical tube fits.
            if length > max_length && max_length > 0.01 {
                diameter = (diameter * 1.35).clamp(0.025, 0.08);
                length = port_length_m(diameter, tuning, bass_volume);
            }
            for _ in 0..10 {
                if length <= max_length && length >= 0.01 {
                    break;
                }
                tuning *= 1.08;
                length = port_length_m(diameter, tuning, bass_volume);
            }
            if length < 0.01 || length > max_length {
                return Err("no port of this tuning fits the cabinet.".into());
            }
            // At the end of the height away from a front driver.
            let low = WALL_M + diameter / 2.0 + 0.012;
            let z = if front && tweeter.is_none() && v < oh / 2.0 {
                oh - low
            } else {
                low
            };
            if front
                && tweeter.is_some()
                && !back
                && v - part.outer_m / 2.0 < z + diameter / 2.0 + 0.01
            {
                return Err("the port does not fit under the bass driver.".into());
            }
            let px = ow / 2.0;
            let path = if egg {
                // Through the middle of the flat back, clear of the driver's magnet.
                if length > od - part.depth_m - 0.03 {
                    return Err("no port of this tuning fits the cabinet.".into());
                }
                json!([[px, od - length, oh / 2.0], [px, od, oh / 2.0]])
            } else if back {
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
            let chamber_h = (part.outer_m + stacked + 0.03).max(0.3 * oh).min(oh - 0.25);
            if chamber_h < part.outer_m + stacked + 0.02 {
                return Err("the cabinet is too short for a line under its drivers.".into());
            }
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
        let chamber_h = (part.outer_m + stacked + 0.03).max(0.3 * oh).min(oh - 0.25);
        [face_w / 2.0, oh - WALL_M - chamber_h / 2.0 - stacked / 2.0]
    } else {
        [face_w / 2.0, v]
    };
    let mut mounts = vec![json!({ "id": "m1", "surface": family.face, "at_m": at,
        "rear_into": "main", "recess_m": 0.0 })];
    let mut radiators = vec![json!({ "kind": "cone", "driver_id": part.id, "mount": "m1" })];
    let chain = |blocks: Value, gain_db: f64| {
        json!({ "blocks": blocks, "delay_s": 0.0, "gain_db": gain_db, "invert": false,
                "limiter": null })
    };
    let mut chains = vec![chain(json!([]), 0.0)];
    if let Some(t) = tweeter {
        let split = |band: &str, f: f64| {
            json!({ "kind": "crossover", "family": "linkwitz-riley", "order": 4, "f_hz": f,
                    "band": band })
        };
        // Where the bass driver hands over: to the tweeter, or in a three-way to the
        // midrange. The handover sits two octaves above the next driver's resonance at the
        // least (it must not be asked for bass), and, where an independent test found the
        // lower driver's cone break up, an octave below that.
        let (low_f, mid_band) = match mid {
            Some(m) => {
                let lowest = (2.0 * m.fs_hz).max(250.0);
                let highest = part.breakup_hz.map_or(900.0, |f| (f / 2.0).min(900.0));
                if highest < lowest {
                    return Err("the midrange cannot reach down to where the woofer stops.".into());
                }
                let f1 = lowest * (highest / lowest).powf(x[13]);
                (f1, Some((m, f1)))
            }
            None => (0.0, None),
        };
        let lowest = (2.0 * t.fs_hz)
            .max(1500.0)
            .max(mid_band.map_or(0.0, |(_, f1)| 2.5 * f1));
        let below = mid_band.map_or(part, |(m, _)| m);
        let highest = below.breakup_hz.map_or(4500.0, |f| (f / 2.0).min(4500.0));
        if highest < lowest {
            return Err(
                "the tweeter cannot reach down to an octave below the lower driver's break-up."
                    .into(),
            );
        }
        let f = lowest * (highest / lowest).powf(x[10]);
        let level = lerp(-12.0, 0.0, x[11]);
        let woofer_top = if mid.is_some() { low_f } else { f };
        chains = vec![chain(json!([split("low", woofer_top)]), 0.0)];
        let tweeter_z;
        if let Some((m, f1)) = mid_band {
            mounts.push(
                json!({ "id": "m3", "surface": "front", "at_m": [ow / 2.0, mid_z],
                "rear_into": "mid", "recess_m": 0.0 }),
            );
            radiators.push(json!({ "kind": "cone", "driver_id": m.id, "mount": "m3" }));
            let mid_level = lerp(-12.0, 0.0, x[14]);
            chains.push(chain(
                json!([split("high", f1), split("low", f)]),
                mid_level,
            ));
            tweeter_z = mid_z + m.outer_m / 2.0 + gap + t.outer_m / 2.0;
            named.push(("lower crossover Hz".into(), f1));
            named.push(("midrange level dB".into(), mid_level));
            summary.push_str(&format!(", {} from {f1:.0} Hz", m.name));
        } else if front {
            tweeter_z = at[1] + part.outer_m / 2.0 + gap + t.outer_m / 2.0;
        } else {
            // The bass driver is on another face: the tweeter goes high on the front.
            tweeter_z = lerp(oh / 2.0, oh - t.outer_m / 2.0 - WALL_M - 0.006, along)
                .max(t.outer_m / 2.0 + WALL_M + 0.006);
        }
        if family.woofers > 1 {
            // The second woofer above the tweeter, in the same chamber, on the same settings.
            let z2 = tweeter_z + t.outer_m / 2.0 + gap + part.outer_m / 2.0;
            mounts.push(
                json!({ "id": "m4", "surface": "front", "at_m": [ow / 2.0, z2],
                "rear_into": "main", "recess_m": 0.0 }),
            );
            radiators.insert(
                1,
                json!({ "kind": "cone", "driver_id": part.id, "mount": "m4" }),
            );
            chains.insert(1, chain(json!([split("low", woofer_top)]), 0.0));
            summary.push_str(", two woofers");
        }
        mounts.push(
            json!({ "id": "m2", "surface": "front", "at_m": [ow / 2.0, tweeter_z],
            "rear_into": if mid.is_some() { "mid" } else { "main" }, "recess_m": 0.0 }),
        );
        radiators.push(json!({ "kind": "cone", "driver_id": t.id, "mount": "m2" }));
        chains.push(chain(json!([split("high", f)]), level));
        named.push(("crossover Hz".into(), f));
        named.push(("tweeter level dB".into(), level));
        summary.push_str(&format!(", {} above {f:.0} Hz", t.name));
    }
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
            "mounts": mounts,
            "solids": [] },
        "radiators": radiators,
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
            "per_radiator": chains,
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
    let parts = if family.ways >= 3 {
        parts_of(&["woofer"])
    } else {
        parts()
    };
    let mids = parts_of(&["midrange", "fullrange"]);
    if family.ways >= 3 && mids.is_empty() {
        return Err("the catalog has no sourced budget midrange with mounting sizes.".into());
    }
    if parts.is_empty() {
        return Err("the catalog has no sourced budget driver with mounting sizes.".into());
    }
    let tweeters = tweeters();
    if family.ways > 1 && tweeters.is_empty() {
        return Err("the catalog has no sourced budget tweeter with mounting sizes.".into());
    }
    let names = |list: &[Part]| {
        list.iter()
            .map(|part| part.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut notes = vec![format!(
        "Drivers tried: {}. Only sourced budget drivers whose records give their mounting sizes \
         are proposed.",
        names(&parts)
    )];
    if family.ways > 1 {
        notes.push(format!(
            "Tweeters tried: {}. The split is a fourth-order Linkwitz-Riley crossover; its \
             frequency and the tweeter's level are searched.",
            names(&tweeters)
        ));
    }
    if family.ways >= 3 {
        notes.push(format!(
            "Midranges tried: {}. Each sits in its own sealed chamber above the woofer's and \
             gets a band between two crossovers.",
            names(&mids)
        ));
    }
    if family.woofers > 1 {
        notes.push(
            "The two woofers share one chamber and the same processing, one above and one \
             below the tweeter."
                .into(),
        );
    }
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
    let mut points = Vec::new();
    let mut best: Vec<(f64, [f64; VARIABLES], Generated)> = Vec::new();
    let mut scored = 0usize;
    let mut thrown_out = 0usize;
    let mut reasons: Vec<(String, usize)> = Vec::new();
    let mut canceled = false;
    // Building is cheap and scoring is not, so a candidate that cannot be built costs a try,
    // not an evaluation.
    let mut tries = 0usize;
    while scored < budget && tries < budget * 40 {
        tries += 1;
        // Three of every five candidates are random and two refine the best so far. Nothing
        // depends on the budget, so a longer search begins with the shorter one's candidates
        // and its best is never worse.
        let x: [f64; VARIABLES] = if scored % 5 < 3 || best.is_empty() {
            std::array::from_fn(|_| random.next())
        } else {
            // Refine one of the best: small moves, smaller as the search goes on, and now and
            // then another driver.
            let from = &best[(random.next() * best.len().min(2) as f64) as usize].1;
            let spread = lerp(0.18, 0.05, (scored as f64 / 40.0).min(1.0));
            std::array::from_fn(|index| {
                if CHOICES.contains(&index) {
                    if random.next() < 0.2 {
                        random.next()
                    } else {
                        from[index]
                    }
                } else {
                    (from[index] + spread * (2.0 * random.next() - 1.0)).clamp(0.0, 1.0)
                }
            })
        };
        let built = match build(base, &family, &parts, &tweeters, &mids, &x, registry) {
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
