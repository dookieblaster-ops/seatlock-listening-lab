//! Several chambers (README sections 5.3 and 6.2). Each chamber is a box of air,
//! `min_m` to `max_m`, inside the cabinet's air. Whatever inside air no chamber claims is
//! printed solid: those are the partition walls. Chambers are the nodes of the air network
//! and ducts its edges. With no chambers listed, the whole inside is one chamber, `main`.

use seatlock_acoustics::network::Duct;
use seatlock_acoustics::{end_correction_m, PortSpec};
use seatlock_core::registry::{AirRegions, MouldedOpening, OuterShape};
use seatlock_core::DesignFile;
use serde_json::Value;

use crate::ports::DEFAULT_PORT_WALL_M;

#[derive(Clone, Debug)]
pub struct Chamber {
    pub id: String,
    pub min_m: [f64; 3],
    pub max_m: [f64; 3],
    /// Plastic placed in this chamber's air (the design's `voxels` solids), m^3.
    pub plastic_m3: f64,
    /// For an imported model with marked holes: the enclosed air space this chamber is, found
    /// from the model's geometry; `min_m` and `max_m` are then only its bounds.
    pub region: Option<(std::sync::Arc<AirRegions>, usize)>,
}

impl Chamber {
    /// The chamber's air: its box (or air space) less the plastic placed in it.
    pub fn volume_m3(&self) -> f64 {
        self.box_m3() - self.plastic_m3
    }
    pub fn box_m3(&self) -> f64 {
        if let Some((air, index)) = &self.region {
            return air.volumes_m3[*index];
        }
        (0..3).map(|i| self.max_m[i] - self.min_m[i]).product()
    }
    pub fn contains(&self, p: [f64; 3]) -> bool {
        if let Some((air, index)) = &self.region {
            return air.region_at(p) == Some(*index);
        }
        (0..3).all(|i| p[i] > self.min_m[i] && p[i] < self.max_m[i])
    }
}

/// The chambers the simulator uses: the design's, or, for an imported model with marked
/// holes, each enclosed air space a sound source or a moulded duct opens into, called
/// `space-1`, `space-2`, ... from the largest.
pub fn for_simulation(design: &DesignFile, shape: &dyn OuterShape) -> Result<Vec<Chamber>, String> {
    let Some(air) = shape.air_regions() else {
        return chambers(design);
    };
    if !design.speaker.enclosure.chambers.is_empty() {
        return Err(
            "an imported cabinet with marked holes finds its own air spaces; leave its chambers \
             list empty."
                .into(),
        );
    }
    let mut used: Vec<usize> = design
        .speaker
        .enclosure
        .mounts
        .iter()
        .filter_map(|mount| shape.region_behind(&mount.surface))
        .chain(shape.moulded_openings().iter().map(|o| o.region))
        .collect();
    used.sort_unstable();
    used.dedup();
    Ok(used
        .into_iter()
        .map(|index| {
            let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
            for (cell, label) in air.labels.iter().enumerate() {
                if *label as usize != index + 1 {
                    continue;
                }
                let c = [
                    cell / (air.n[1] * air.n[2]),
                    (cell / air.n[2]) % air.n[1],
                    cell % air.n[2],
                ];
                for k in 0..3 {
                    let x = air.origin_m[k] + c[k] as f64 * air.dx_m;
                    lo[k] = lo[k].min(x);
                    hi[k] = hi[k].max(x + air.dx_m);
                }
            }
            Chamber {
                id: format!("space-{}", index + 1),
                min_m: lo,
                max_m: hi,
                plastic_m3: 0.0,
                region: Some((air.clone(), index)),
            }
        })
        .collect())
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
        out.push(Chamber {
            id,
            min_m,
            max_m,
            plastic_m3: 0.0,
            region: None,
        });
    }
    let (placed, _) = crate::solids::solids(design)?;
    for chamber in &mut out {
        let inside =
            |p: [f64; 3]| (0..3).all(|i| p[i] > chamber.min_m[i] && p[i] < chamber.max_m[i]);
        chamber.plastic_m3 = placed.iter().map(|solid| solid.plastic_m3(&inside)).sum();
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
    /// For each radiator in design order, the chamber it pushes into, when it loads the
    /// cabinet.
    pub driver_nodes: Vec<Option<usize>>,
    /// Ducts moulded into an imported model, each with the chamber it leads from; they follow
    /// the design's ducts to the outside in `ports`.
    pub moulded: Vec<(MouldedOpening, usize)>,
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
    let list = match for_simulation(design, shape) {
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
        if chamber.region.is_some() {
            continue;
        }
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
    let notes = Vec::new();

    // Drivers: each takes its own air from the chamber behind it, and each that loads the
    // cabinet must name that chamber.
    let mut driver = None;
    let mut driver_nodes = Vec::new();
    for part in &design.speaker.radiators {
        driver_nodes.push(None);
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
        let node = if chambers.iter().any(|c| c.region.is_some()) {
            shape.region_behind(&mount.surface).and_then(|region| {
                chambers
                    .iter()
                    .position(|c| c.region.as_ref().is_some_and(|(_, r)| *r == region))
            })
        } else {
            index_of(&mount.rear_into)
        };
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
        if let Some(slot) = driver_nodes.last_mut() {
            *slot = Some(node);
        }
        if driver.is_none() {
            driver = Some(node);
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
    // Ducts moulded into an imported model: their air is in their space's volume.
    let mut moulded = Vec::new();
    for opening in shape.moulded_openings() {
        let node = chambers
            .iter()
            .position(|c| c.region.as_ref().is_some_and(|(_, r)| *r == opening.region))
            .ok_or(format!("Opening {} leads from no chamber.", opening.id))?;
        volumes[node] -= opening.volume_m3();
        let length = opening.length_m();
        let r_in = (opening.inner_area_m2() / std::f64::consts::PI).sqrt();
        let r_out = (opening.mouth_area_m2() / std::f64::consts::PI).sqrt();
        let duct = Duct {
            from: node,
            to: None,
            area_m2: length / opening.length_over_area(),
            length_eff_m: length + end_correction_m(r_in, true) + end_correction_m(r_out, false),
        };
        ports.push(PortSpec {
            area_m2: opening.mouth_area_m2(),
            length_eff_m: duct.length_eff_m,
        });
        ducts.push(duct);
        moulded.push((opening, node));
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
        driver_nodes,
        moulded,
        notes,
    })
}

/// The `waveguide` model of the air network (README 5.4): from the driver's chamber, each
/// duct is a transmission line along its center line (a bend counts as straight, a taper is
/// sliced), ending in the radiation load of its mouth or in the chamber at its far end, and
/// that chamber in turn in its own ducts. Chambers are compliances. A network with a loop
/// is not a tree, and is refused with the reason.
pub struct WaveguideNetwork {
    pub root: seatlock_acoustics::AcousticNode,
    /// For each opening the solved result lists, in its order, the index of that duct among
    /// the ducts to the outside, in design order.
    pub opening_order: Vec<usize>,
    pub notes: Vec<String>,
}

/// `names` and `volumes_m3` are the chambers (one called `main` when the design lists none),
/// `driver` the chamber the driver pushes into.
pub fn waveguide_network(
    design: &DesignFile,
    names: &[String],
    volumes_m3: &[f64],
    driver: usize,
    moulded: &[(MouldedOpening, usize)],
) -> Result<WaveguideNetwork, String> {
    use seatlock_acoustics::AcousticNode;
    struct Edge {
        a: Option<usize>,
        b: Option<usize>,
        section: Section,
        length: f64,
        outside_slot: Option<usize>,
        /// A moulded duct's slices from the chamber to the mouth: `(length, area)`.
        slices: Option<Vec<(f64, f64)>>,
    }
    let index_of = |name: &str| -> Result<Option<usize>, String> {
        if name == "outside" {
            return Ok(None);
        }
        names
            .iter()
            .position(|n| n == name)
            .map(Some)
            .ok_or(format!(
                "a duct names {name}, which is not one of the chambers."
            ))
    };
    let mut edges = Vec::new();
    let mut outside_count = 0;
    for raw in &design.speaker.enclosure.ducts {
        let id = raw.get("id").and_then(Value::as_str).unwrap_or("duct");
        let a = index_of(raw.get("from").and_then(Value::as_str).unwrap_or(""))?;
        let b = index_of(raw.get("to").and_then(Value::as_str).unwrap_or(""))?;
        if a.is_none() && b.is_none() {
            return Err(format!("duct {id} runs from outside to outside."));
        }
        let section = Section::of(raw).map_err(|e| format!("duct {id}: {e}"))?;
        let length = path_length(&path(raw));
        if length <= 0.0 {
            return Err(format!("duct {id} has no path."));
        }
        let outside_slot = if a.is_none() || b.is_none() {
            outside_count += 1;
            Some(outside_count - 1)
        } else {
            None
        };
        edges.push(Edge {
            a,
            b,
            section,
            length,
            outside_slot,
            slices: None,
        });
    }
    // Moulded ducts, from their chamber to the mouth, in slices of about 10 mm (README 5.4).
    for (opening, node) in moulded {
        let mut pieces: Vec<(f64, f64)> = opening
            .segment_lengths_m()
            .into_iter()
            .zip(opening.areas_m2.iter().copied())
            .rev()
            .collect();
        let mut merged: Vec<(f64, f64)> = Vec::new();
        let (mut length, mut l_over_a) = (0.0, 0.0);
        for (l, a) in pieces.drain(..) {
            length += l;
            l_over_a += l / a.max(1e-12);
            if length >= 0.01 {
                merged.push((length, length / l_over_a));
                length = 0.0;
                l_over_a = 0.0;
            }
        }
        if length > 0.0 {
            merged.push((length, length / l_over_a));
        }
        let total: f64 = merged.iter().map(|m| m.0).sum();
        outside_count += 1;
        edges.push(Edge {
            a: Some(*node),
            b: None,
            section: Section {
                round: true,
                start: vec![(4.0 * opening.inner_area_m2() / std::f64::consts::PI).sqrt()],
                end: vec![(4.0 * opening.mouth_area_m2() / std::f64::consts::PI).sqrt()],
                tube_m: 0.0,
            },
            length: total,
            outside_slot: Some(outside_count - 1),
            slices: Some(merged),
        });
    }
    let mut order = Vec::new();
    let mut seen = vec![false; names.len()];
    let mut used = vec![false; edges.len()];
    fn chamber_node(
        at: usize,
        volumes: &[f64],
        edges: &[Edge],
        seen: &mut Vec<bool>,
        used: &mut Vec<bool>,
        order: &mut Vec<usize>,
    ) -> Result<AcousticNode, String> {
        seen[at] = true;
        let mut branches = vec![AcousticNode::Chamber {
            volume_m3: volumes[at],
            slot: at,
        }];
        for (index, edge) in edges.iter().enumerate() {
            if used[index] || (edge.a != Some(at) && edge.b != Some(at)) {
                continue;
            }
            used[index] = true;
            // Orient the duct from this chamber.
            let forward = edge.a == Some(at);
            let far = if forward { edge.b } else { edge.a };
            let (s_near, s_far) = if forward { (0.0, 1.0) } else { (1.0, 0.0) };
            let (a_near, a_far) = (
                edge.section.area(s_near, 0.0),
                edge.section.area(s_far, 0.0),
            );
            let r_near = (a_near / std::f64::consts::PI).sqrt();
            let r_far = (a_far / std::f64::consts::PI).sqrt();
            let (load, far_end) = match far {
                None => {
                    order.push(edge.outside_slot.unwrap_or(0));
                    (
                        AcousticNode::Radiation {
                            area_m2: a_far,
                            flush: false,
                        },
                        0.0,
                    )
                }
                Some(other) => {
                    if seen[other] {
                        return Err(
                            "its chambers and ducts form a loop, which the detailed model does \
                             not solve yet."
                                .into(),
                        );
                    }
                    (
                        chamber_node(other, volumes, edges, seen, used, order)?,
                        0.849 * r_far,
                    )
                }
            };
            if let Some(slices) = &edge.slices {
                // From the mouth back to the chamber, each slice loaded by the next.
                let mut node = load;
                for (index, (length, area)) in slices.iter().enumerate().rev() {
                    let added = if index == 0 {
                        0.849 * (area / std::f64::consts::PI).sqrt()
                    } else {
                        0.0
                    };
                    node = AcousticNode::Duct {
                        length_m: *length,
                        area_in_m2: *area,
                        area_out_m2: *area,
                        added_length_m: added,
                        load: Box::new(node),
                    };
                }
                branches.push(node);
                continue;
            }
            branches.push(AcousticNode::Duct {
                length_m: edge.length,
                area_in_m2: a_near,
                area_out_m2: a_far,
                added_length_m: 0.849 * r_near + far_end,
                load: Box::new(load),
            });
        }
        Ok(AcousticNode::Parallel(branches))
    }
    let root = chamber_node(driver, volumes_m3, &edges, &mut seen, &mut used, &mut order)?;
    let mut notes = Vec::new();
    if used.iter().any(|u| !u) {
        notes.push(
            "Some ducts do not connect to the driver's chamber, so the detailed model leaves \
             them out."
                .into(),
        );
    }
    let bends: usize = design
        .speaker
        .enclosure
        .ducts
        .iter()
        .map(|raw| path(raw).len().saturating_sub(2))
        .sum();
    if bends > 0 {
        notes.push(format!(
            "{bends} bend{} in the ducts treated as straight: each duct is its center line's \
             length, and the air in a corner is not modeled.",
            if bends == 1 { "" } else { "s" }
        ));
    }
    for (opening, _) in moulded {
        notes.push(format!(
            "Port {} is moulded into the imported model: measured on its grid as {:.0} mm long \
             along its middle, {:.1} cm^2 at the mouth and {:.1} cm^2 on average; its bends \
             count as straight.",
            opening.id,
            opening.length_m() * 1000.0,
            opening.mouth_area_m2() * 1e4,
            opening.length_m() / opening.length_over_area() * 1e4
        ));
    }
    notes.push(
        "Chambers are single compliances, so standing waves inside them are not modeled.".into(),
    );
    Ok(WaveguideNetwork {
        root,
        opening_order: order,
        notes,
    })
}
