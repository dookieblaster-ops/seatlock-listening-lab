//! The optimizer (README section 11.3): the cheapest design that meets the target.
//! Discrete choices (sealed or vented, crossover slope, and — for a single cone — catalog
//! driver) are combos; each combo searches box volume, tuning, crossover frequency, and
//! gains with Nelder-Mead, correcting and scoring every candidate.
//!
//! When the design has a passive R/L/C board (`dsp.passive`), the search keeps the cabinet
//! and the netlist wiring fixed and varies only the part values toward the score (README
//! section 7). Digital correction is left out: with a passive board those filters are not
//! used on the graphs.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use seatlock_core::registry::Registry;
use seatlock_core::{catalog, DesignFile, C0};

use crate::compare::{rebuild_to, score, score_as_is, Body, Score};

#[derive(Debug, Clone, Deserialize)]
pub struct OptimizeRequest {
    #[serde(default)]
    pub seed: u64,
    /// Which combo to run; see [`combos`].
    #[serde(default)]
    pub combo: usize,
    /// Candidates to score in this combo.
    #[serde(default = "default_evaluations")]
    pub evaluations: usize,
}

fn default_evaluations() -> usize {
    30
}

#[derive(Debug, Clone, Serialize)]
pub struct Combo {
    pub index: usize,
    pub label: String,
    pub vented: bool,
    /// Linkwitz-Riley order of the crossovers, for designs with more than one radiator.
    pub crossover_order: Option<u32>,
    /// When set, the first cone radiator is swapped to this catalog id before the search
    /// (Level 3: driver choice). Multi-way and passive boards leave this unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_id: Option<String>,
}

/// One scored candidate, with the variables that made it.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub combo: usize,
    pub summary: String,
    pub score: Score,
    pub design: DesignFile,
    pub variables: Vec<(String, f64)>,
}

/// Every candidate's cost and quality, for the cost-against-quality plot.
#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub cost_usd: f64,
    pub shortfall: f64,
    pub meets_target: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComboResult {
    pub combo: Combo,
    pub points: Vec<Point>,
    /// The best few, best first.
    pub best: Vec<Candidate>,
    pub evaluations: usize,
    pub canceled: bool,
    pub notes: Vec<String>,
}

fn multi_way(design: &DesignFile) -> bool {
    design.speaker.radiators.len() > 1
}

fn has_passive(design: &DesignFile) -> bool {
    design.dsp.passive.is_some()
}

pub fn combos(design: &DesignFile) -> Vec<Combo> {
    // A passive board: only its R/L/C values are searched (cabinet and wiring stay put).
    if has_passive(design) {
        return vec![Combo {
            index: 0,
            label: "Passive board values".into(),
            vented: false,
            crossover_order: None,
            driver_id: None,
        }];
    }
    let tube = design.speaker.enclosure.outer.kind == "tube";
    let drivers = driver_choices(design);
    let mut out = Vec::new();
    for driver in &drivers {
        for vented in [false, true] {
            if vented && tube {
                continue;
            }
            let orders: Vec<Option<u32>> = if multi_way(design) {
                vec![Some(4), Some(2)]
            } else {
                vec![None]
            };
            for order in orders {
                let mut label = if vented { "Vented" } else { "Sealed" }.to_string();
                if let Some(order) = order {
                    label.push_str(&format!(", LR{order} crossover"));
                }
                if let Some((id, name)) = driver {
                    if design
                        .speaker
                        .radiators
                        .first()
                        .and_then(|r| r.fields.get("driver_id"))
                        .and_then(Value::as_str)
                        != Some(id.as_str())
                    {
                        label.push_str(&format!(", {name}"));
                    }
                }
                out.push(Combo {
                    index: out.len(),
                    label,
                    vented,
                    crossover_order: order,
                    driver_id: driver.as_ref().map(|(id, _)| id.clone()),
                });
            }
        }
    }
    out
}

/// Discrete driver choices for Level 3. Multi-way keeps the user's pair (`None` once).
/// A single cone tries the current driver plus nearby sourced budget drivers with mounting
/// sizes (capped), never `test-*`.
fn driver_choices(design: &DesignFile) -> Vec<Option<(String, String)>> {
    if multi_way(design) {
        return vec![None];
    }
    let Some(part) = design.speaker.radiators.first() else {
        return vec![None];
    };
    if part.kind != "cone" {
        return vec![None];
    }
    let Some(current_id) = part.fields.get("driver_id").and_then(Value::as_str) else {
        return vec![None];
    };
    let current_name = catalog::driver(current_id)
        .map(|record| {
            let maker = record.get("maker").and_then(Value::as_str).unwrap_or("");
            let model = record
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(current_id);
            format!("{maker} {model}").trim().to_string()
        })
        .unwrap_or_else(|| current_id.to_string());
    let current_sd = catalog::driver(current_id)
        .and_then(|r| r.pointer("/published/sd_m2").and_then(Value::as_f64))
        .unwrap_or(0.01);
    let mut alts: Vec<(f64, String, String)> = catalog::all("catalog/drivers/")
        .into_iter()
        .filter_map(|record| {
            let text = |key: &str| record.get(key).and_then(Value::as_str);
            if text("status") != Some("sourced") || text("tier") != Some("budget") {
                return None;
            }
            let kind = text("kind")?;
            if !matches!(kind, "woofer" | "fullrange" | "midrange") {
                return None;
            }
            let id = text("id")?;
            if id == current_id || id.starts_with("test-") {
                return None;
            }
            record
                .pointer("/mounting/cutout_diameter_m")
                .and_then(Value::as_f64)
                .filter(|v| *v > 0.0)?;
            record
                .pointer("/mounting/outer_diameter_m")
                .and_then(Value::as_f64)
                .filter(|v| *v > 0.0)?;
            let sd = record.pointer("/published/sd_m2").and_then(Value::as_f64)?;
            let name = format!("{} {}", text("maker")?, text("model")?);
            let distance = (sd / current_sd).ln().abs();
            Some((distance, id.to_string(), name))
        })
        .collect();
    alts.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    // Current first, then up to three nearest by cone area (keeps the page's combo count small).
    let mut out = vec![Some((current_id.to_string(), current_name))];
    for (_, id, name) in alts.into_iter().take(3) {
        out.push(Some((id, name)));
    }
    out
}

/// A search variable on `[0, 1]`, mapped to `[low, high]` on a log or a linear scale.
#[derive(Debug, Clone)]
struct Variable {
    name: String,
    low: f64,
    high: f64,
    log: bool,
}

impl Variable {
    fn value(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        if self.log {
            self.low * (self.high / self.low).powf(x)
        } else {
            self.low + (self.high - self.low) * x
        }
    }
    fn position(&self, value: f64) -> f64 {
        let x = if self.log {
            (value / self.low).ln() / (self.high / self.low).ln()
        } else {
            (value - self.low) / (self.high - self.low)
        };
        x.clamp(0.0, 1.0)
    }
}

fn published(design: &DesignFile, key: &str) -> Option<f64> {
    let id = design
        .speaker
        .radiators
        .first()?
        .fields
        .get("driver_id")?
        .as_str()?;
    catalog::driver(id)?
        .pointer(&format!("/published/{key}"))?
        .as_f64()
}

/// The design's current port, if one leaves to outside: (diameter, [x, z] of its path).
fn current_port(design: &DesignFile) -> Option<(f64, [f64; 2])> {
    design.speaker.enclosure.ducts.iter().find_map(|duct| {
        let to = duct.get("to").and_then(Value::as_str)?;
        let from = duct.get("from").and_then(Value::as_str)?;
        if to != "outside" && from != "outside" {
            return None;
        }
        let section = duct.get("section")?;
        if section.get("shape").and_then(Value::as_str) != Some("round") {
            return None;
        }
        let diameter = section.pointer("/start_m/0")?.as_f64()?;
        let first = duct.pointer("/path_m/0")?.as_array()?;
        Some((
            diameter,
            [first.first()?.as_f64()?, first.get(2)?.as_f64()?],
        ))
    })
}

struct Space {
    variables: Vec<Variable>,
    start: Vec<f64>,
}

fn space(design: &DesignFile, combo: &Combo, volume_now: f64) -> Space {
    let vas = published(design, "vas_m3").unwrap_or(volume_now.max(0.005));
    let fs = published(design, "fs_hz").unwrap_or(50.0);
    let low = (0.1 * vas).min(volume_now * 0.5).max(0.0015);
    let high = (2.0 * vas).max(volume_now * 2.0).min(0.08);
    let mut variables = vec![Variable {
        name: "net air volume (L)".into(),
        low,
        high,
        log: true,
    }];
    let mut start = vec![volume_now.clamp(low, high)];
    if combo.vented {
        variables.push(Variable {
            name: "port tuning (Hz)".into(),
            low: 0.6 * fs,
            high: 1.6 * fs,
            log: true,
        });
        start.push(fs);
    }
    if combo.crossover_order.is_some() {
        let now = crossover_hz(design).unwrap_or(2500.0);
        variables.push(Variable {
            name: "crossover (Hz)".into(),
            low: 800.0,
            high: 5000.0,
            log: true,
        });
        start.push(now.clamp(800.0, 5000.0));
        for (index, chain) in design.dsp.per_radiator.iter().enumerate().skip(1) {
            variables.push(Variable {
                name: format!("gain of driver {} (dB)", index + 1),
                low: -12.0,
                high: 6.0,
                log: false,
            });
            start.push(chain.gain_db.clamp(-12.0, 6.0));
        }
    }
    let start = start
        .iter()
        .zip(&variables)
        .map(|(value, variable)| variable.position(*value))
        .collect();
    Space { variables, start }
}

fn crossover_hz(design: &DesignFile) -> Option<f64> {
    design
        .dsp
        .per_radiator
        .iter()
        .flat_map(|chain| chain.blocks.iter())
        .find(|block| block.kind == "crossover")
        .and_then(|block| block.fields.get("f_hz").and_then(Value::as_f64))
}

/// Port length for a tuning: `L = C0^2 S / ((2 pi f_b)^2 V)` less one flush and one free end
/// correction (README section 5.3).
pub(crate) fn port_length_m(diameter: f64, tuning_hz: f64, volume_m3: f64) -> f64 {
    let area = std::f64::consts::PI * diameter * diameter / 4.0;
    let omega = 2.0 * std::f64::consts::PI * tuning_hz;
    let l_eff = C0 * C0 * area / (omega * omega * volume_m3);
    let a = diameter / 2.0;
    l_eff - (0.849 + 0.613) * a
}

fn candidate_design(
    base: &DesignFile,
    combo: &Combo,
    variables: &[Variable],
    x: &[f64],
    registry: &Registry,
) -> Result<(DesignFile, Vec<(String, f64)>), String> {
    let mut design = base.clone();
    for chain in design.dsp.per_radiator.iter_mut() {
        chain
            .blocks
            .retain(|block| block.fields.get("correction") != Some(&Value::Bool(true)));
    }
    let values: Vec<f64> = variables
        .iter()
        .zip(x)
        .map(|(variable, x)| variable.value(*x))
        .collect();
    let mut named: Vec<(String, f64)> = Vec::new();
    let mut cursor = 0;
    let volume = values[cursor];
    named.push((variables[cursor].name.clone(), volume * 1000.0));
    cursor += 1;
    // Ports to outside are replaced by the one this combo asks for.
    let old_port = current_port(base);
    design.speaker.enclosure.ducts.retain(|duct| {
        duct.get("to").and_then(Value::as_str) != Some("outside")
            && duct.get("from").and_then(Value::as_str) != Some("outside")
    });
    if combo.vented {
        let tuning = values[cursor];
        named.push((variables[cursor].name.clone(), tuning));
        cursor += 1;
        let sd = published(base, "sd_m2").unwrap_or(0.01);
        let cone = (4.0 * sd / std::f64::consts::PI).sqrt();
        let diameter = old_port
            .map(|(d, _)| d)
            .unwrap_or((0.35 * cone).clamp(0.025, 0.08));
        let length = port_length_m(diameter, tuning, volume).max(0.01);
        let wall = base
            .speaker
            .enclosure
            .outer
            .fields
            .get("wall_m")
            .and_then(Value::as_f64)
            .unwrap_or(0.012);
        let place = old_port
            .map(|(_, at)| at)
            .unwrap_or([wall + diameter / 2.0 + 0.01, wall + diameter / 2.0 + 0.01]);
        design.speaker.enclosure.ducts.push(json!({
            "id": "port",
            "from": "main",
            "to": "outside",
            "path_m": [[place[0], length, place[1]], [place[0], 0.0, place[1]]],
            "section": { "shape": "round", "start_m": [diameter], "end_m": [diameter] }
        }));
    }
    if let Some(order) = combo.crossover_order {
        let frequency = values[cursor];
        named.push((variables[cursor].name.clone(), frequency));
        cursor += 1;
        for chain in design.dsp.per_radiator.iter_mut() {
            for block in chain
                .blocks
                .iter_mut()
                .filter(|block| block.kind == "crossover")
            {
                block.fields.insert("f_hz".into(), json!(frequency));
                block.fields.insert("order".into(), json!(order));
                block
                    .fields
                    .insert("family".into(), json!("linkwitz-riley"));
            }
        }
        for chain in design.dsp.per_radiator.iter_mut().skip(1) {
            if cursor < values.len() {
                chain.gain_db = values[cursor];
                named.push((variables[cursor].name.clone(), values[cursor]));
                cursor += 1;
            }
        }
    }
    let (rebuilt, _) = rebuild_to(&design, &Body::of(base), volume, registry)?;
    // The port must fit inside the cabinet's depth.
    if combo.vented {
        let depth = rebuilt
            .speaker
            .enclosure
            .outer
            .fields
            .get("depth_m")
            .and_then(Value::as_f64)
            .unwrap_or(f64::MAX);
        let length = rebuilt
            .speaker
            .enclosure
            .ducts
            .last()
            .and_then(|duct| duct.pointer("/path_m/0/1"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if length > depth - 0.05 {
            return Err(format!(
                "the port ({:.0} mm) does not fit the cabinet's depth ({:.0} mm).",
                length * 1000.0,
                depth * 1000.0
            ));
        }
    }
    Ok((rebuilt, named))
}

/// Ranks candidates (README section 11.2): meeting the target and cheaper first; then smaller
/// shortfall.
pub(crate) fn objective(score: &Score) -> f64 {
    if !score.problems.is_empty() {
        return 1e12;
    }
    if score.meets_target {
        score.cost_usd
    } else {
        1e6 + 1e3 * score.shortfall + score.cost_usd * 1e-3
    }
}

/// One R, L, or C part in the passive netlist that the value search may change.
#[derive(Debug, Clone)]
struct PassivePart {
    index: usize,
    kind: char,
    /// SI value: ohm, henry, or farad.
    now: f64,
}

fn passive_parts(passive: &Value) -> Result<Vec<PassivePart>, String> {
    let elements = passive
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| "the passive board has no elements list.".to_string())?;
    let mut parts = Vec::new();
    for (index, element) in elements.iter().enumerate() {
        let kind = element
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_uppercase();
        let (kind, key) = match kind.as_str() {
            "R" => ('R', "ohm"),
            "L" => ('L', "henry"),
            "C" => ('C', "farad"),
            _ => continue,
        };
        let now = element
            .get(key)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| {
                format!(
                    "passive part {} ({kind}) needs a positive {key} value.",
                    index + 1
                )
            })?;
        parts.push(PassivePart { index, kind, now });
    }
    if parts.is_empty() {
        return Err(
            "the passive board has no R, L, or C parts to tune (only driver loads).".into(),
        );
    }
    Ok(parts)
}

fn passive_space(parts: &[PassivePart]) -> Space {
    let mut variables = Vec::new();
    for part in parts {
        let (name, low_abs, high_abs) = match part.kind {
            'R' => (format!("R{} (ohm)", part.index + 1), 0.05, 100.0),
            'L' => (format!("L{} (mH)", part.index + 1), 1e-5, 0.05),
            'C' => (format!("C{} (µF)", part.index + 1), 1e-8, 2e-4),
            _ => unreachable!(),
        };
        // Search a factor of four either side of the kit value, clipped to a sane absolute band.
        let low = (part.now * 0.25).max(low_abs);
        let high = (part.now * 4.0).min(high_abs).max(low * 1.01);
        variables.push(Variable {
            name,
            low,
            high,
            log: true,
        });
    }
    let start: Vec<f64> = parts
        .iter()
        .zip(&variables)
        .map(|(part, variable)| variable.position(part.now))
        .collect();
    Space { variables, start }
}

fn apply_passive_values(
    base: &DesignFile,
    parts: &[PassivePart],
    variables: &[Variable],
    x: &[f64],
) -> Result<(DesignFile, Vec<(String, f64)>), String> {
    let mut design = base.clone();
    let passive = design
        .dsp
        .passive
        .as_mut()
        .ok_or_else(|| "the design has no passive board.".to_string())?;
    let elements = match passive.get_mut("elements") {
        Some(Value::Array(list)) => list,
        _ => return Err("the passive board has no elements list.".into()),
    };
    let mut named = Vec::new();
    for ((part, variable), xi) in parts.iter().zip(variables).zip(x) {
        let value = variable.value(*xi);
        let element = elements
            .get_mut(part.index)
            .ok_or_else(|| format!("passive part {} is missing.", part.index + 1))?;
        let object = element
            .as_object_mut()
            .ok_or_else(|| format!("passive part {} is not an object.", part.index + 1))?;
        // Clear sibling unit keys so ohm/henry/farad stay unambiguous.
        object.remove("ohm");
        object.remove("henry");
        object.remove("farad");
        let (key, display) = match part.kind {
            'R' => ("ohm", value),
            'L' => ("henry", value * 1000.0),
            'C' => ("farad", value * 1e6),
            _ => unreachable!(),
        };
        object.insert(key.into(), json!(value));
        named.push((variable.name.clone(), display));
    }
    Ok((design, named))
}

fn default_passive_evaluations(n_parts: usize) -> usize {
    // Simplex needs n+1 points; give a few more steps for boards with many parts.
    (4 * n_parts + 20).clamp(30, 120)
}

/// Searches R/L/C values of `dsp.passive` toward the target score (cabinet fixed).
fn optimize_passive(
    design: &DesignFile,
    request: &OptimizeRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<ComboResult, String> {
    let combo = Combo {
        index: request.combo,
        label: "Passive board values".into(),
        vented: false,
        crossover_order: None,
        driver_id: None,
    };
    let passive = design
        .dsp
        .passive
        .as_ref()
        .ok_or_else(|| "the design has no passive board.".to_string())?;
    let parts = passive_parts(passive)?;
    let Space { variables, start } = passive_space(&parts);
    let mut notes = vec![
        "Cabinet, drivers, and board wiring stay fixed; only R, L, and C values move.".into(),
        "Digital correction is not applied: with a passive board those filters are left out \
         of the graphs."
            .into(),
    ];
    if design.speaker.radiators.iter().any(|part| {
        part.fields
            .get("driver_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("test-"))
    }) {
        notes.push("This design uses test drivers, which are not real products.".into());
    }

    let mut random =
        SplitMix(request.seed ^ (request.combo as u64 + 1).wrapping_mul(0x5851_F42D_4C95_7F2D));
    let budget = if request.evaluations == default_evaluations() {
        default_passive_evaluations(parts.len())
    } else {
        request.evaluations.max(parts.len() + 2)
    };

    let mut points = Vec::new();
    let mut kept: Vec<Candidate> = Vec::new();
    let mut count = 0usize;
    let mut canceled = false;
    let mut failures: Vec<String> = Vec::new();

    let evaluate = |x: &[f64],
                    count: &mut usize,
                    kept: &mut Vec<Candidate>,
                    points: &mut Vec<Point>,
                    failures: &mut Vec<String>|
     -> f64 {
        *count += 1;
        match apply_passive_values(design, &parts, &variables, x) {
            Ok((candidate, named)) => {
                // score_as_is: correction filters would be ignored under dsp.passive.
                let (score, _) = score_as_is(&candidate, registry);
                let value = objective(&score);
                points.push(Point {
                    cost_usd: score.cost_usd,
                    shortfall: score.shortfall,
                    meets_target: score.meets_target,
                });
                let summary = named
                    .iter()
                    .take(6)
                    .map(|(name, value)| format!("{name} {value:.3}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let summary = if named.len() > 6 {
                    format!("{summary}, … ({} parts)", named.len())
                } else {
                    summary
                };
                let close = |other: &Candidate| {
                    other.variables.iter().zip(&named).all(|((_, a), (_, b))| {
                        (a - b).abs() <= 0.03 * a.abs().max(b.abs()).max(1.0)
                    })
                };
                if let Some(index) = kept.iter().position(close) {
                    if objective(&kept[index].score) <= value {
                        return value;
                    }
                    kept.remove(index);
                }
                kept.push(Candidate {
                    combo: combo.index,
                    summary: format!("{}: {summary}", combo.label),
                    score,
                    design: candidate,
                    variables: named,
                });
                kept.sort_by(|a, b| objective(&a.score).total_cmp(&objective(&b.score)));
                kept.truncate(3);
                value
            }
            Err(error) => {
                if failures.len() < 3 && !failures.contains(&error) {
                    failures.push(error);
                }
                1e12
            }
        }
    };

    let n = start.len();
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::new();
    let first = start.clone();
    let value = evaluate(&first, &mut count, &mut kept, &mut points, &mut failures);
    simplex.push((first, value));
    for axis in 0..n {
        let mut x = start.clone();
        let step = 0.15 + 0.1 * random.next();
        x[axis] = if x[axis] + step <= 1.0 {
            x[axis] + step
        } else {
            x[axis] - step
        };
        let value = evaluate(&x, &mut count, &mut kept, &mut points, &mut failures);
        simplex.push((x, value));
        if !progress(count, budget) {
            canceled = true;
            break;
        }
    }
    let clamp = |x: Vec<f64>| -> Vec<f64> { x.into_iter().map(|v| v.clamp(0.0, 1.0)).collect() };
    while !canceled && count < budget {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        let worst = simplex[n].clone();
        let centroid: Vec<f64> = (0..n)
            .map(|axis| simplex[..n].iter().map(|(x, _)| x[axis]).sum::<f64>() / n as f64)
            .collect();
        let along = |t: f64| -> Vec<f64> {
            clamp(
                centroid
                    .iter()
                    .zip(&worst.0)
                    .map(|(c, w)| c + t * (c - w))
                    .collect(),
            )
        };
        let reflected = along(1.0);
        let r = evaluate(
            &reflected,
            &mut count,
            &mut kept,
            &mut points,
            &mut failures,
        );
        if r < simplex[0].1 {
            let expanded = along(2.0);
            let e = evaluate(&expanded, &mut count, &mut kept, &mut points, &mut failures);
            simplex[n] = if e < r { (expanded, e) } else { (reflected, r) };
        } else if r < simplex[n - 1].1 {
            simplex[n] = (reflected, r);
        } else {
            let contracted = along(-0.5);
            let c = evaluate(
                &contracted,
                &mut count,
                &mut kept,
                &mut points,
                &mut failures,
            );
            if c < worst.1 {
                simplex[n] = (contracted, c);
            } else {
                let best = simplex[0].0.clone();
                for entry in simplex.iter_mut().skip(1) {
                    let x: Vec<f64> = entry
                        .0
                        .iter()
                        .zip(&best)
                        .map(|(x, b)| b + 0.5 * (x - b) + 0.02 * (random.next() - 0.5))
                        .collect();
                    let x = clamp(x);
                    let value = evaluate(&x, &mut count, &mut kept, &mut points, &mut failures);
                    *entry = (x, value);
                    if count >= budget {
                        break;
                    }
                }
            }
        }
        if !progress(count, budget) {
            canceled = true;
        }
    }
    for failure in failures {
        notes.push(format!("Some candidates could not be scored: {failure}"));
    }
    if let Some(best) = kept.first() {
        let (start_score, _) = score_as_is(design, registry);
        if best.score.shortfall + 1e-9 < start_score.shortfall {
            let pref = |s: &crate::compare::Score| {
                s.preference
                    .map(|p| format!("{p:.2}"))
                    .unwrap_or_else(|| "—".into())
            };
            notes.push(format!(
                "Best shortfall {:.2} (was {:.2}); preference {} (was {}).",
                best.score.shortfall,
                start_score.shortfall,
                pref(&best.score),
                pref(&start_score)
            ));
        } else {
            notes.push(format!(
                "No better shortfall than the starting board ({:.2}).",
                start_score.shortfall
            ));
        }
    }
    Ok(ComboResult {
        combo,
        points,
        best: kept,
        evaluations: count,
        canceled,
        notes,
    })
}

/// A small, seeded random source, so a run repeats from its seed.
pub(crate) struct SplitMix(pub(crate) u64);

impl SplitMix {
    pub(crate) fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub fn optimize_combo(
    design: &DesignFile,
    request: &OptimizeRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<ComboResult, String> {
    if has_passive(design) {
        return optimize_passive(design, request, registry, progress);
    }
    let all = combos(design);
    let combo = all
        .get(request.combo)
        .cloned()
        .ok_or_else(|| format!("there is no combo {}.", request.combo))?;
    let mut notes = Vec::new();
    let mut design = design.clone();
    if let Some(id) = &combo.driver_id {
        let Some(radiator) = design.speaker.radiators.first_mut() else {
            return Err("there is no radiator to change.".into());
        };
        let previous = radiator
            .fields
            .get("driver_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if previous != *id {
            radiator.fields.insert("driver_id".into(), json!(id));
            // Correction was for the previous driver; this combo starts clean.
            for chain in design.dsp.per_radiator.iter_mut() {
                chain
                    .blocks
                    .retain(|block| block.fields.get("correction") != Some(&Value::Bool(true)));
            }
            notes.push(format!("Driver set to {id} (was {previous})."));
        }
    }
    if design.speaker.radiators.iter().any(|part| {
        part.fields
            .get("driver_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("test-"))
    }) {
        notes.push("This design uses test drivers, which are not real products.".into());
    }
    if !design.speaker.enclosure.chambers.is_empty() {
        return Err("the optimizer does not handle several chambers yet.".into());
    }
    if !matches!(
        design.speaker.enclosure.outer.kind.as_str(),
        "box" | "sheet_box" | "tube"
    ) {
        notes.push(
            "Candidates are printed boxes; the optimizer does not reshape this outer shape yet."
                .into(),
        );
    }
    let (now, _) = score(&design, registry);
    let volume_now = now.interior_volume_m3.unwrap_or(0.01);
    let Space { variables, start } = space(&design, &combo, volume_now);
    let mut random =
        SplitMix(request.seed ^ (request.combo as u64 + 1).wrapping_mul(0x5851_F42D_4C95_7F2D));

    let budget = request.evaluations.max(4);
    let mut points = Vec::new();
    let mut kept: Vec<Candidate> = Vec::new();
    let mut count = 0usize;
    let mut canceled = false;
    let mut failures: Vec<String> = Vec::new();

    let evaluate = |x: &[f64],
                    count: &mut usize,
                    kept: &mut Vec<Candidate>,
                    points: &mut Vec<Point>,
                    failures: &mut Vec<String>|
     -> f64 {
        *count += 1;
        match candidate_design(&design, &combo, &variables, x, registry) {
            Ok((candidate, named)) => {
                let (score, corrected) = score(&candidate, registry);
                let value = objective(&score);
                points.push(Point {
                    cost_usd: score.cost_usd,
                    shortfall: score.shortfall,
                    meets_target: score.meets_target,
                });
                let summary = named
                    .iter()
                    .map(|(name, value)| format!("{name} {value:.1}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                // Keep the best few that differ: a near copy of a kept candidate replaces it
                // only when it is better.
                let close = |other: &Candidate| {
                    other.variables.iter().zip(&named).all(|((_, a), (_, b))| {
                        (a - b).abs() <= 0.03 * a.abs().max(b.abs()).max(1.0)
                    })
                };
                if let Some(index) = kept.iter().position(close) {
                    if objective(&kept[index].score) <= value {
                        return value;
                    }
                    kept.remove(index);
                }
                kept.push(Candidate {
                    combo: combo.index,
                    summary: format!("{}: {summary}", combo.label),
                    score,
                    design: corrected,
                    variables: named,
                });
                kept.sort_by(|a, b| objective(&a.score).total_cmp(&objective(&b.score)));
                kept.truncate(3);
                value
            }
            Err(error) => {
                if failures.len() < 3 && !failures.contains(&error) {
                    failures.push(error);
                }
                1e12
            }
        }
    };

    // Nelder-Mead on [0, 1]^n, starting from the design's own values.
    let n = start.len();
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::new();
    let first = start.clone();
    let value = evaluate(&first, &mut count, &mut kept, &mut points, &mut failures);
    simplex.push((first, value));
    for axis in 0..n {
        let mut x = start.clone();
        let step = 0.15 + 0.1 * random.next();
        x[axis] = if x[axis] + step <= 1.0 {
            x[axis] + step
        } else {
            x[axis] - step
        };
        let value = evaluate(&x, &mut count, &mut kept, &mut points, &mut failures);
        simplex.push((x, value));
        if !progress(count, budget) {
            canceled = true;
            break;
        }
    }
    let clamp = |x: Vec<f64>| -> Vec<f64> { x.into_iter().map(|v| v.clamp(0.0, 1.0)).collect() };
    while !canceled && count < budget {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        let worst = simplex[n].clone();
        let centroid: Vec<f64> = (0..n)
            .map(|axis| simplex[..n].iter().map(|(x, _)| x[axis]).sum::<f64>() / n as f64)
            .collect();
        let along = |t: f64| -> Vec<f64> {
            clamp(
                centroid
                    .iter()
                    .zip(&worst.0)
                    .map(|(c, w)| c + t * (c - w))
                    .collect(),
            )
        };
        let reflected = along(1.0);
        let r = evaluate(
            &reflected,
            &mut count,
            &mut kept,
            &mut points,
            &mut failures,
        );
        if r < simplex[0].1 {
            let expanded = along(2.0);
            let e = evaluate(&expanded, &mut count, &mut kept, &mut points, &mut failures);
            simplex[n] = if e < r { (expanded, e) } else { (reflected, r) };
        } else if r < simplex[n - 1].1 {
            simplex[n] = (reflected, r);
        } else {
            let contracted = along(-0.5);
            let c = evaluate(
                &contracted,
                &mut count,
                &mut kept,
                &mut points,
                &mut failures,
            );
            if c < worst.1 {
                simplex[n] = (contracted, c);
            } else {
                // Shrink toward the best, with a little seeded jitter so a flat region does not
                // stall the search.
                let best = simplex[0].0.clone();
                for entry in simplex.iter_mut().skip(1) {
                    let x: Vec<f64> = entry
                        .0
                        .iter()
                        .zip(&best)
                        .map(|(x, b)| b + 0.5 * (x - b) + 0.02 * (random.next() - 0.5))
                        .collect();
                    let x = clamp(x);
                    let value = evaluate(&x, &mut count, &mut kept, &mut points, &mut failures);
                    *entry = (x, value);
                    if count >= budget {
                        break;
                    }
                }
            }
        }
        if !progress(count, budget) {
            canceled = true;
        }
    }
    for failure in failures {
        notes.push(format!("Some candidates could not be built: {failure}"));
    }
    Ok(ComboResult {
        combo,
        points,
        best: kept,
        evaluations: count,
        canceled,
        notes,
    })
}
