//! Several chambers (README sections 5.3 and 6.2). Each chamber is a box of air,
//! `min_m` to `max_m`, inside the cabinet's air. Whatever inside air no chamber claims is
//! printed solid: those are the partition walls. Chambers are the nodes of the air network
//! and ducts its edges. With no chambers listed, the whole inside is one chamber, `main`.

use seatlock_acoustics::network::Duct;
use seatlock_acoustics::{end_correction_m, PortSpec};
use seatlock_core::registry::OuterShape;
use seatlock_core::DesignFile;
use serde_json::Value;

use crate::ports::DEFAULT_PORT_WALL_M;

#[derive(Clone, Debug)]
pub struct Chamber {
    pub id: String,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
}

impl Chamber {
    pub fn volume_m3(&self) -> f64 {
        (0..3).map(|i| self.max_m[i] - self.min_m[i]).product()
    }
    pub fn contains(&self, p: [f64; 3]) -> bool {
        (0..3).all(|i| p[i] > self.min_m[i] && p[i] < self.max_m[i])
    }
}

fn triple(value: Option<&Value>) -> Option<[f64; 3]> {
    let items = value?.as_array()?;
    Some([
        items.first()?.as_f64()?,
        items.get(1)?.as_f64()?,
        items.get(2)?.as_f64()?,
    ])
}

/// The design's chambers. Empty when the design lists none.
pub fn chambers(design: &DesignFile) -> Result<Vec<Chamber>, String> {
    let mut out: Vec<Chamber> = Vec::new();
    for (index, raw) in design.speaker.enclosure.chambers.iter().enumerate() {
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .ok_or(format!("Chamber {} has no id.", index + 1))?
            .to_string();
        let min_m = triple(raw.get("min_m")).ok_or(format!("Chamber {id} has no min_m."))?;
        let max_m = triple(raw.get("max_m")).ok_or(format!("Chamber {id} has no max_m."))?;
        if (0..3).any(|i| max_m[i] <= min_m[i]) {
            return Err(format!("Chamber {id} has no size."));
        }
        if out.iter().any(|other| other.id == id) || id == "outside" {
            return Err(format!("Two chambers are called {id}."));
        }
        out.push(Chamber { id, min_m, max_m });
    }
    Ok(out)
}

/// Lengths along the segment from `a` to `b` that lie in some chamber, as `(from, to)` in m
/// from `a`, sorted and merged. With no chambers, the whole segment.
pub fn air_intervals(a: [f64; 3], b: [f64; 3], chambers: &[Chamber]) -> Vec<(f64, f64)> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if chambers.is_empty() {
        return vec![(0.0, length)];
    }
    let mut spans = Vec::new();
    for chamber in chambers {
        // Slab method on the parameter s in [0, 1].
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        let mut empty = false;
        for i in 0..3 {
            if d[i].abs() < 1e-15 {
                if a[i] <= chamber.min_m[i] || a[i] >= chamber.max_m[i] {
                    empty = true;
                }
            } else {
                let s0 = (chamber.min_m[i] - a[i]) / d[i];
                let s1 = (chamber.max_m[i] - a[i]) / d[i];
                lo = lo.max(s0.min(s1));
                hi = hi.min(s0.max(s1));
            }
        }
        if !empty && hi > lo {
            spans.push((lo * length, hi * length));
        }
    }
    spans.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.0 <= last.1 => last.1 = last.1.max(span.1),
            _ => merged.push(span),
        }
    }
    merged
}

/// A duct's cross-section: its area at a fraction `s` of the way along, and the outside
/// area of its printed tube.
pub struct Section {
    pub round: bool,
    pub start: Vec<f64>,
    pub end: Vec<f64>,
    pub tube_m: f64,
}

impl Section {
    pub fn of(duct: &Value) -> Result<Section, String> {
        let section = duct.get("section").ok_or("it has no section.")?;
        let size = |key: &str| -> Vec<f64> {
            section
                .get(key)
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default()
        };
        let start = size("start_m");
        let mut end = size("end_m");
        if end.is_empty() {
            end = start.clone();
        }
        let round = section
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or("round")
            == "round";
        let wanted = if round { 1 } else { 2 };
        if start.len() < wanted || end.len() < wanted || start.iter().chain(&end).any(|v| *v <= 0.0)
        {
            return Err("its section has no size.".into());
        }
        Ok(Section {
            round,
            start,
            end,
            tube_m: duct
                .get("wall_m")
                .and_then(Value::as_f64)
                .unwrap_or(DEFAULT_PORT_WALL_M),
        })
    }

    fn dims(&self, s: f64, grow: f64) -> Vec<f64> {
        self.start
            .iter()
            .zip(&self.end)
            .map(|(a, b)| a + (b - a) * s + 2.0 * grow)
            .collect()
    }

    /// Area at fraction `s`, with the walls grown by `grow` on every side.
    pub fn area(&self, s: f64, grow: f64) -> f64 {
        let d = self.dims(s, grow);
        if self.round {
            std::f64::consts::PI * d[0] * d[0] / 4.0
        } else {
            d[0] * d[1]
        }
    }

    /// Exact volume between fractions `s0` and `s1` of a duct `length` long: the area is a
    /// quadratic in `s`, so Simpson's rule is exact.
    pub fn volume(&self, length: f64, s0: f64, s1: f64, grow: f64) -> f64 {
        let mid = 0.5 * (s0 + s1);
        length * (s1 - s0) / 6.0
            * (self.area(s0, grow) + 4.0 * self.area(mid, grow) + self.area(s1, grow))
    }

    /// The air mass length over area, `integral of dx / S(x)`, for the lumped model. Exact for
    /// a round taper (`L / (pi * r0 * r1)`); a slot keeps one size.
    pub fn inertance_length_over_area(&self, length: f64) -> f64 {
        if self.round {
            length / (std::f64::consts::PI * self.start[0] / 2.0 * self.end[0] / 2.0)
        } else {
            length / self.area(0.0, 0.0)
        }
    }

    /// The radius for end corrections: of the round end, or of a circle of the slot's area.
    pub fn end_radius(&self, at_end: bool) -> f64 {
        (self.area(if at_end { 1.0 } else { 0.0 }, 0.0) / std::f64::consts::PI).sqrt()
    }
}

fn path(duct: &Value) -> Vec<[f64; 3]> {
    duct.get("path_m")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|item| triple(Some(item))).collect())
        .unwrap_or_default()
}

fn path_length(points: &[[f64; 3]]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let d = [
                pair[1][0] - pair[0][0],
                pair[1][1] - pair[0][1],
                pair[1][2] - pair[0][2],
            ];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .sum()
}

/// The several-chamber air network for the `lumped` model.
pub struct AirNetwork {
    pub chambers: Vec<Chamber>,
    /// Net air per chamber: its box less the drivers behind it and the duct tubes in it.
    pub volumes_m3: Vec<f64>,
    pub ducts: Vec<Duct>,
    /// The ducts to the outside, as the vented code reads them, in design order.
    pub ports: Vec<PortSpec>,
    /// The chamber the first driver that loads the cabinet pushes into.
    pub driver: usize,
    pub notes: Vec<String>,
}

/// `None` when the design lists no chambers; otherwise the network, or why it cannot be
/// built.
pub fn air_network(
    design: &DesignFile,
    shape: &dyn OuterShape,
    displacement_of: impl Fn(&seatlock_core::design::KindPart) -> f64,
    loads_box: impl Fn(&seatlock_core::design::KindPart) -> bool,
) -> Option<Result<AirNetwork, String>> {
    let list = match chambers(design) {
        Ok(list) if list.is_empty() => return None,
        Ok(list) => list,
        Err(err) => return Some(Err(err)),
    };
    Some(build(design, shape, list, displacement_of, loads_box))
}

fn build(
    design: &DesignFile,
    shape: &dyn OuterShape,
    chambers: Vec<Chamber>,
    displacement_of: impl Fn(&seatlock_core::design::KindPart) -> f64,
    loads_box: impl Fn(&seatlock_core::design::KindPart) -> bool,
) -> Result<AirNetwork, String> {
    for (i, chamber) in chambers.iter().enumerate() {
        let inset = 1e-6;
        for corner in 0..8 {
            let p = [
                if corner & 1 == 0 {
                    chamber.min_m[0] + inset
                } else {
                    chamber.max_m[0] - inset
                },
                if corner & 2 == 0 {
                    chamber.min_m[1] + inset
                } else {
                    chamber.max_m[1] - inset
                },
                if corner & 4 == 0 {
                    chamber.min_m[2] + inset
                } else {
                    chamber.max_m[2] - inset
                },
            ];
            if !shape.contains_air(p)? {
                return Err(format!(
                    "Chamber {} reaches outside the cabinet's inside air.",
                    chamber.id
                ));
            }
        }
        for other in &chambers[i + 1..] {
            if (0..3)
                .all(|k| chamber.min_m[k] < other.max_m[k] && other.min_m[k] < chamber.max_m[k])
            {
                return Err(format!("Chambers {} and {} overlap.", chamber.id, other.id));
            }
        }
    }
    let index_of = |id: &str| chambers.iter().position(|chamber| chamber.id == id);
    let mut volumes: Vec<f64> = chambers.iter().map(Chamber::volume_m3).collect();
    let mut notes = Vec::new();

    // Drivers: each takes its own air from the chamber behind it, and each that loads the
    // cabinet must name that chamber.
    let mut driver = None;
    for part in &design.speaker.radiators {
        let mount_id = part
            .fields
            .get("mount")
            .and_then(Value::as_str)
            .unwrap_or("");
        let Some(mount) = design
            .speaker
            .enclosure
            .mounts
            .iter()
            .find(|mount| mount.id == mount_id)
        else {
            continue;
        };
        let node = index_of(&mount.rear_into);
        if let Some(node) = node {
            volumes[node] -= displacement_of(part);
        }
        if !loads_box(part) {
            continue;
        }
        let node = node.ok_or(format!(
            "Mount {} opens into {}, which is not one of the chambers.",
            mount.id, mount.rear_into
        ))?;
        match driver {
            None => driver = Some(node),
            Some(first) if first != node => notes.push(
                "Drivers in different chambers are each solved as if the other were still."
                    .to_string(),
            ),
            _ => {}
        }
    }
    let driver = driver.ok_or("No driver opens into a chamber.")?;

    // Ducts: edges of the network. Their tubes take air from the chambers they pass through.
    let mut ducts = Vec::new();
    let mut ports = Vec::new();
    for raw in &design.speaker.enclosure.ducts {
        let id = raw.get("id").and_then(Value::as_str).unwrap_or("duct");
        let from = raw.get("from").and_then(Value::as_str).unwrap_or("");
        let to = raw.get("to").and_then(Value::as_str).unwrap_or("");
        let node = |name: &str| -> Result<Option<usize>, String> {
            if name == "outside" {
                return Ok(None);
            }
            index_of(name).map(Some).ok_or(format!(
                "Duct {id} names {name}, which is not one of the chambers."
            ))
        };
        let (a, b) = (node(from)?, node(to)?);
        let (inside, other) = match (a, b) {
            (Some(x), y) => (x, y),
            (None, Some(y)) => (y, None),
            (None, None) => return Err(format!("Duct {id} runs from outside to outside.")),
        };
        let section = Section::of(raw).map_err(|e| format!("Duct {id}: {e}"))?;
        let points = path(raw);
        let length = path_length(&points);
        if points.len() < 2 || length <= 0.0 {
            return Err(format!("Duct {id} has no path."));
        }
        let mut done = 0.0;
        for pair in points.windows(2) {
            let seg = path_length(pair);
            for chamber_index in 0..chambers.len() {
                for (t0, t1) in
                    air_intervals(pair[0], pair[1], &chambers[chamber_index..=chamber_index])
                {
                    let (s0, s1) = ((done + t0) / length, (done + t1) / length);
                    volumes[chamber_index] -= section.volume(length, s0, s1, section.tube_m);
                }
            }
            done += seg;
        }
        let ends = match other {
            Some(_) => {
                end_correction_m(section.end_radius(false), true)
                    + end_correction_m(section.end_radius(true), true)
            }
            None => {
                end_correction_m(section.end_radius(from == "outside"), true)
                    + end_correction_m(section.end_radius(from != "outside"), false)
            }
        };
        // The lumped mass uses `rho * L_eff / S`; carry the taper in an equivalent area.
        let area = length / section.inertance_length_over_area(length);
        let duct = Duct {
            from: inside,
            to: other,
            area_m2: area,
            length_eff_m: length + ends,
        };
        if other.is_none() {
            ports.push(PortSpec {
                area_m2: section.area(if from == "outside" { 0.0 } else { 1.0 }, 0.0),
                length_eff_m: duct.length_eff_m,
            });
        }
        ducts.push(duct);
    }
    if let Some((index, _)) = volumes.iter().enumerate().find(|(_, v)| **v <= 0.0) {
        return Err(format!(
            "Chamber {} has no air left after its drivers and tubes.",
            chambers[index].id
        ));
    }
    if ports.len() > 4 {
        return Err("More than four ducts lead outside; the lab handles four so far.".into());
    }
    Ok(AirNetwork {
        chambers,
        volumes_m3: volumes,
        ducts,
        ports,
        driver,
        notes,
    })
}
