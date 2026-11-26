//! Several chambers (README sections 5.3 and 6.2). Each chamber is a box of air,
//! `min_m` to `max_m`, inside the cabinet's air. Whatever inside air no chamber claims is
//! printed solid: those are the partition walls. Chambers are the nodes of the air network
//! and ducts its edges. With no chambers listed, the whole inside is one chamber, `main`.

use seatlock_acoustics::network::Duct;
use seatlock_acoustics::{
    end_correction_m, MeshDuct, PassiveRadiatorMech, PortSpec, WaveguideMesh,
};
use seatlock_core::catalog;
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
    /// Catalog damping material id filling this chamber's air (`catalog/damping/`), when set.
    pub fill: Option<String>,
    /// Flow resistivity of `fill` in Pa·s/m², resolved from the catalog when `fill` is set.
    pub fill_sigma: Option<f64>,
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
                fill: None,
                fill_sigma: None,
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
        let fill = match raw.get("fill") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| {
                        format!("Chamber {id}: fill must be a damping material id or null.")
                    })?
                    .to_string(),
            ),
        };
        let fill_sigma = match &fill {
            None => None,
            Some(material) => {
                let record = seatlock_core::catalog::damping(material).ok_or_else(|| {
                    format!(
                        "Chamber {id}: fill {material} is not a damping material in the catalog."
                    )
                })?;
                let sigma = record
                    .get("flow_resistivity_pa_s_m2")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| {
                        format!(
                            "Chamber {id}: damping material {material} has no flow resistivity."
                        )
                    })?;
                if sigma <= 0.0 {
                    return Err(format!(
                        "Chamber {id}: damping material {material} has a non-positive flow \
                         resistivity."
                    ));
                }
                Some(sigma)
            }
        };
        out.push(Chamber {
            id,
            min_m,
            max_m,
            plastic_m3: 0.0,
            fill,
            fill_sigma,
            region: None,
        });
    }
    let placed = crate::solids::solids(design)?;
    for chamber in &mut out {
        let inside =
            |p: [f64; 3]| (0..3).all(|i| p[i] > chamber.min_m[i] && p[i] < chamber.max_m[i]);
        chamber.plastic_m3 = placed.plastic_m3(&inside);
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
#[derive(Clone, Debug)]
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
        self.inertance_length_over_area_between(length, 0.0, 1.0)
    }

    /// Same integral over the fraction range `[s0, s1]` of a duct `length` long.
    pub fn inertance_length_over_area_between(&self, length: f64, s0: f64, s1: f64) -> f64 {
        let (a, b) = if s1 >= s0 { (s0, s1) } else { (s1, s0) };
        let span = (b - a).max(0.0);
        if span <= 0.0 {
            return 0.0;
        }
        if self.round {
            let r0 = self.dims(a, 0.0)[0] / 2.0;
            let r1 = self.dims(b, 0.0)[0] / 2.0;
            length * span / (std::f64::consts::PI * r0.max(1e-9) * r1.max(1e-9))
        } else {
            // Slot: area is linear in s; use the trapezoid mean.
            let mean = 0.5 * (self.area(a, 0.0) + self.area(b, 0.0)).max(1e-12);
            length * span / mean
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
    /// Duct id → chamber name for a driver mounted on that duct (README §6.2).
    pub duct_from_override: Vec<(String, String)>,
    /// Duct id → tap fraction along the duct from the closed/`from` end (0 = closed end).
    /// Absent ducts are closed-end mounts. King / Augspurger offset driver (README §6.2).
    pub duct_taps: Vec<(String, f64)>,
    pub notes: Vec<String>,
}

/// Default coupling volume when a driver mounts on a duct with no listed chamber (0.1 L).
pub const DUCT_COUPLING_M3: f64 = 1.0e-4;

/// `None` when the design lists no chambers and no duct-mounted driver; otherwise the
/// network, or why it cannot be built.
pub fn air_network(
    design: &DesignFile,
    shape: &dyn OuterShape,
    displacement_of: impl Fn(&seatlock_core::design::KindPart) -> f64,
    loads_box: impl Fn(&seatlock_core::design::KindPart) -> bool,
) -> Option<Result<AirNetwork, String>> {
    let list = match for_simulation(design, shape) {
        Ok(list) if list.is_empty() => {
            if duct_mounted_drivers(design).is_empty() {
                return None;
            }
            Vec::new()
        }
        Ok(list) => list,
        Err(err) => return Some(Err(err)),
    };
    Some(build(design, shape, list, displacement_of, loads_box))
}

/// Mounts whose `rear_into` names a duct id (not a chamber): `(mount_id, duct_id,
/// coupling_volume_m3, tap_fraction)`. `tap_fraction` is 0 at the closed/`from` end and
/// approaches 1 at the mouth (README §6.2; King offset driver).
pub fn duct_mounted_drivers(design: &DesignFile) -> Vec<(String, String, f64, f64)> {
    let chamber_ids: Vec<&str> = design
        .speaker
        .enclosure
        .chambers
        .iter()
        .filter_map(|c| c.get("id").and_then(Value::as_str))
        .collect();
    let duct_ids: Vec<&str> = design
        .speaker
        .enclosure
        .ducts
        .iter()
        .filter_map(|d| d.get("id").and_then(Value::as_str))
        .collect();
    let mut out = Vec::new();
    for mount in &design.speaker.enclosure.mounts {
        let target = mount.rear_into.as_str();
        if chamber_ids.contains(&target) {
            continue;
        }
        if !duct_ids.contains(&target) {
            continue;
        }
        let coupling = mount
            .extra
            .get("coupling_volume_m3")
            .and_then(Value::as_f64)
            .filter(|v| *v > 0.0)
            .unwrap_or(DUCT_COUPLING_M3);
        let tap = mount
            .extra
            .get("tap_fraction")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 0.95);
        out.push((mount.id.clone(), target.to_string(), coupling, tap));
    }
    out
}

fn build(
    design: &DesignFile,
    shape: &dyn OuterShape,
    mut chambers: Vec<Chamber>,
    displacement_of: impl Fn(&seatlock_core::design::KindPart) -> f64,
    loads_box: impl Fn(&seatlock_core::design::KindPart) -> bool,
) -> Result<AirNetwork, String> {
    for (i, chamber) in chambers.iter().enumerate() {
        if chamber.region.is_some() || is_coupling_chamber(&chamber.id) {
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
            if is_coupling_chamber(&other.id) {
                continue;
            }
            if (0..3)
                .all(|k| chamber.min_m[k] < other.max_m[k] && other.min_m[k] < chamber.max_m[k])
            {
                return Err(format!("Chambers {} and {} overlap.", chamber.id, other.id));
            }
        }
    }
    let mut volumes: Vec<f64> = chambers.iter().map(Chamber::volume_m3).collect();
    let mut notes = Vec::new();
    let mut duct_from_override: Vec<(String, String)> = Vec::new();

    // Drivers mounted on a duct: a small coupling volume at the tap (closed end when
    // tap_fraction is 0). King / Augspurger offset: closed stub behind the tap in parallel
    // with the open section to the mouth (README 6.2).
    let duct_mounts = duct_mounted_drivers(design);
    let mut duct_claim: Vec<(String, usize)> = Vec::new();
    let mut duct_taps: Vec<(String, f64)> = Vec::new();
    for (mount_id, duct_id, coupling, tap) in &duct_mounts {
        if duct_claim.iter().any(|(d, _)| d == duct_id) {
            return Err(format!(
                "Two mounts claim duct {duct_id}; only one driver may sit on a duct in this build."
            ));
        }
        let couple_id = coupling_chamber_id(duct_id);
        if chambers.iter().any(|c| c.id == couple_id) {
            return Err(format!(
                "Chamber id {couple_id} is reserved for a duct mount."
            ));
        }
        chambers.push(Chamber {
            id: couple_id.clone(),
            min_m: [0.0; 3],
            max_m: [0.0; 3],
            plastic_m3: 0.0,
            fill: None,
            fill_sigma: None,
            region: None,
        });
        volumes.push(*coupling);
        let node = chambers.len() - 1;
        duct_claim.push((duct_id.clone(), node));
        duct_from_override.push((duct_id.clone(), couple_id));
        duct_taps.push((duct_id.clone(), *tap));
        if *tap > 1e-9 {
            notes.push(format!(
                "Mount {mount_id} sits on duct {duct_id} at {:.0}% of its length from the closed \
                 end: the everyday models use a {:.2} L coupling volume there, a closed stub \
                 behind the tap, and the open section to the mouth (King / Augspurger offset).",
                tap * 100.0,
                coupling * 1000.0
            ));
        } else {
            notes.push(format!(
                "Mount {mount_id} sits on duct {duct_id}: the everyday models use a {:.2} L \
                 coupling volume at the duct's closed end (not the whole cabinet air).",
                coupling * 1000.0
            ));
        }
    }

    let index_of = |id: &str| chambers.iter().position(|chamber| chamber.id == id);

    // Drivers: each takes its own air from the chamber behind it, or from a duct-mount
    // coupling volume.
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
        let duct_node = duct_mounts
            .iter()
            .find(|(id, _, _, _)| id == &mount.id)
            .and_then(|(_, duct_id, _, _)| {
                duct_claim
                    .iter()
                    .find(|(d, _)| d == duct_id)
                    .map(|(_, n)| *n)
            });
        let node = if let Some(n) = duct_node {
            Some(n)
        } else if chambers.iter().any(|c| c.region.is_some()) {
            shape.region_behind(&mount.surface).and_then(|region| {
                chambers
                    .iter()
                    .position(|c| c.region.as_ref().is_some_and(|(_, r)| *r == region))
            })
        } else {
            index_of(&mount.rear_into)
        };
        if let Some(node) = node {
            volumes[node] = (volumes[node] - displacement_of(part)).max(1e-9);
        }
        if !loads_box(part) {
            continue;
        }
        let node = node.ok_or(format!(
            "Mount {} opens into {}, which is not one of the chambers or ducts.",
            mount.id, mount.rear_into
        ))?;
        if let Some(slot) = driver_nodes.last_mut() {
            *slot = Some(node);
        }
        if driver.is_none() {
            driver = Some(node);
        }
    }
    let driver = driver.ok_or("No driver opens into a chamber or duct.")?;

    // Ducts: edges of the network. Their tubes take air from the chambers they pass through.
    // A duct of kind `passive_radiator` is a Small Map/Cap/Rap branch to the outside (README 5).
    // A duct claimed by a mount starts from that mount's coupling volume.
    let mut ducts = Vec::new();
    let mut ports = Vec::new();
    for raw in &design.speaker.enclosure.ducts {
        let id = raw.get("id").and_then(Value::as_str).unwrap_or("duct");
        let from = raw.get("from").and_then(Value::as_str).unwrap_or("");
        let to = raw.get("to").and_then(Value::as_str).unwrap_or("");
        let claimed = duct_claim.iter().find(|(d, _)| d == id).map(|(_, n)| *n);
        let node = |name: &str| -> Result<Option<usize>, String> {
            if name == "outside" {
                return Ok(None);
            }
            index_of(name).map(Some).ok_or(format!(
                "Duct {id} names {name}, which is not one of the chambers."
            ))
        };
        let (inside, other) = if let Some(n) = claimed {
            let to_node = node(to)?;
            if to_node.is_some() {
                return Err(format!(
                    "Duct {id}: a driver-mounted duct must open to the outside in this build."
                ));
            }
            (n, None)
        } else {
            let (a, b) = (node(from)?, node(to)?);
            match (a, b) {
                (Some(x), y) => (x, y),
                (None, Some(y)) => (y, None),
                (None, None) => {
                    return Err(format!("Duct {id} runs from outside to outside."));
                }
            }
        };
        if raw.get("kind").and_then(Value::as_str) == Some("passive_radiator") {
            if other.is_some() {
                return Err(format!(
                    "Duct {id}: a passive radiator must open to the outside."
                ));
            }
            let pr = passive_radiator_of(raw).map_err(|e| format!("Duct {id}: {e}"))?;
            let points = path(raw);
            if points.len() < 2 {
                return Err(format!(
                    "Duct {id}: a passive radiator needs a path through the wall it leaves by."
                ));
            }
            // The diaphragm sits in the wall: no tube air is subtracted from the chamber.
            let duct = Duct {
                from: inside,
                to: None,
                area_m2: pr.sd_m2,
                length_eff_m: 0.0,
                passive: Some(pr),
            };
            ports.push(PortSpec {
                area_m2: duct.area_m2,
                length_eff_m: 0.0,
            });
            notes.push(format!(
                "Passive radiator {id}: Small Map/Cap/Rap from its moving mass, compliance, \
                 and resistance (Sd {:.1} cm²).",
                pr.sd_m2 * 1e4
            ));
            ducts.push(duct);
            continue;
        }
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
        let tap = duct_taps
            .iter()
            .find(|(d, _)| d == id)
            .map(|(_, t)| *t)
            .unwrap_or(0.0);
        // Offset tap (King): lumped merges the closed stub's air into the coupling volume and
        // keeps only the open section as the air-mass duct to the outside. Waveguide splits
        // properly (closed stub ‖ open section).
        let (s_open0, open_length) = if claimed.is_some() && tap > 1e-9 {
            let stub_vol = section.volume(length, 0.0, tap, 0.0);
            volumes[inside] += stub_vol;
            notes.push(format!(
                "Quick network: the closed stub's {:.2} L of air is added to the coupling \
                 volume; the detailed model keeps the stub as a closed transmission line.",
                stub_vol * 1000.0
            ));
            (tap, length * (1.0 - tap))
        } else {
            (0.0, length)
        };
        let ends = match other {
            Some(_) => {
                end_correction_m(section.end_radius(false), true)
                    + end_correction_m(section.end_radius(true), true)
            }
            None => {
                let r_in = (section.area(s_open0, 0.0) / std::f64::consts::PI).sqrt();
                let r_out = section.end_radius(from != "outside");
                end_correction_m(r_in, true) + end_correction_m(r_out, false)
            }
        };
        // The lumped mass uses `rho * L_eff / S`; carry the taper in an equivalent area.
        let lo_a = section.inertance_length_over_area_between(length, s_open0, 1.0);
        let area = open_length / lo_a.max(1e-12);
        let duct = Duct {
            from: inside,
            to: other,
            area_m2: area,
            length_eff_m: open_length + ends,
            passive: None,
        };
        if other.is_none() {
            // Same equivalent area as the duct: vented_sample / the 3D self-check need the
            // inertance area, not the mouth's face (a flare would understate the air mass).
            ports.push(PortSpec {
                area_m2: duct.area_m2,
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
            passive: None,
        };
        ports.push(PortSpec {
            area_m2: duct.area_m2,
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
    Ok(AirNetwork {
        chambers,
        volumes_m3: volumes,
        ducts,
        ports,
        driver,
        driver_nodes,
        moulded,
        duct_from_override,
        duct_taps,
        notes,
    })
}

fn coupling_chamber_id(duct_id: &str) -> String {
    format!("_couple_{duct_id}")
}

fn is_coupling_chamber(id: &str) -> bool {
    id.starts_with("_couple_")
}

/// The `waveguide` model of the air network (README 5.4): from the driver's chamber, each
/// duct is a transmission line along its center line (a bend counts as straight, a taper is
/// sliced), ending in the radiation load of its mouth or in the chamber at its far end, and
/// that chamber in turn in its own ducts. Chambers are compliances. A network with a loop
/// is solved as a nodal mesh of the same duct Y-parameters (Level 3).
pub struct WaveguideNetwork {
    pub body: WaveguideBody,
    /// For each opening the solved result lists, in its order, the index of that duct among
    /// the ducts to the outside, in design order.
    pub opening_order: Vec<usize>,
    pub notes: Vec<String>,
}

pub enum WaveguideBody {
    /// Tree grown from the driver's chamber (no loops).
    Tree(seatlock_acoustics::AcousticNode),
    /// General chamber–duct graph, including loops.
    Mesh(WaveguideMesh),
}

impl WaveguideNetwork {
    pub fn solve(
        &self,
        freq_hz: f64,
        lossy: bool,
        walls: &[num_complex::Complex64],
    ) -> seatlock_acoustics::WaveguideResult {
        match &self.body {
            WaveguideBody::Tree(root) => {
                seatlock_acoustics::solve_with_walls(root, freq_hz, lossy, walls)
            }
            WaveguideBody::Mesh(mesh) => {
                seatlock_acoustics::solve_mesh(mesh, freq_hz, lossy, walls, None)
            }
        }
    }

    /// Analytic mouths, or a full BEM mouth impedance matrix (mutual loads iterated).
    pub fn solve_radiation(
        &self,
        freq_hz: f64,
        z_rad: Option<&[Vec<num_complex::Complex64>]>,
        lossy: bool,
        walls: &[num_complex::Complex64],
    ) -> seatlock_acoustics::WaveguideResult {
        match (z_rad, &self.body) {
            (Some(z), WaveguideBody::Tree(root)) => {
                seatlock_acoustics::solve_with_radiation_matrix(root, freq_hz, z, lossy, walls)
            }
            (Some(z), WaveguideBody::Mesh(mesh)) => {
                let n = z.len();
                if n == 0 {
                    return seatlock_acoustics::solve_mesh(mesh, freq_hz, lossy, walls, None);
                }
                let mut loads: Vec<_> = (0..n)
                    .map(|i| z[i].get(i).copied().unwrap_or_default())
                    .collect();
                let mut solved =
                    seatlock_acoustics::solve_mesh(mesh, freq_hz, lossy, walls, Some(&loads));
                for _ in 0..8 {
                    if solved.openings.len() != n {
                        break;
                    }
                    let next = seatlock_acoustics::effective_radiation_loads(z, &solved.openings);
                    let mut settled = true;
                    for (old, new) in loads.iter().zip(next.iter()) {
                        let scale = old.norm().max(new.norm()).max(1e-12);
                        if (old - new).norm() / scale > 1e-6 {
                            settled = false;
                            break;
                        }
                    }
                    loads = next;
                    solved =
                        seatlock_acoustics::solve_mesh(mesh, freq_hz, lossy, walls, Some(&loads));
                    if settled {
                        break;
                    }
                }
                solved
            }
            (None, _) => self.solve(freq_hz, lossy, walls),
        }
    }
}

/// `names` and `volumes_m3` are the chambers (one called `main` when the design lists none),
/// `driver` the chamber the driver pushes into. `duct_from_override` remaps a duct's chamber
/// end when a driver mounts on that duct. `duct_taps` is the tap fraction from the closed end
/// (King offset); the closed stub is a rigid-terminated branch in parallel with the open
/// section.
#[allow(clippy::too_many_arguments)]
pub fn waveguide_network(
    design: &DesignFile,
    names: &[String],
    volumes_m3: &[f64],
    fill_sigma: &[Option<f64>],
    driver: usize,
    moulded: &[(MouldedOpening, usize)],
    duct_from_override: &[(String, String)],
    duct_taps: &[(String, f64)],
) -> Result<WaveguideNetwork, String> {
    use seatlock_acoustics::AcousticNode;
    struct Edge {
        a: Option<usize>,
        b: Option<usize>,
        section: Section,
        length: f64,
        /// Fraction along the full design duct for the near and far ends of this edge
        /// (0 = closed/`from`, 1 = mouth/`to`). Used for tapers on offset-tap splits.
        s0: f64,
        s1: f64,
        outside_slot: Option<usize>,
        /// A moulded duct's slices from the chamber to the mouth: `(length, area)`.
        slices: Option<Vec<(f64, f64)>>,
        /// Small Map/Cap/Rap outside branch (README §5); length/section unused then.
        passive: Option<PassiveRadiatorMech>,
    }
    /// Closed stub behind an offset tap: duct from chamber `at` to a rigid end.
    struct RigidStub {
        at: usize,
        section: Section,
        length: f64,
        s0: f64,
        s1: f64,
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
    let mut rigid_stubs = Vec::new();
    let mut outside_count = 0;
    for raw in &design.speaker.enclosure.ducts {
        let id = raw.get("id").and_then(Value::as_str).unwrap_or("duct");
        let from_name = duct_from_override
            .iter()
            .find(|(d, _)| d == id)
            .map(|(_, c)| c.as_str())
            .unwrap_or_else(|| raw.get("from").and_then(Value::as_str).unwrap_or(""));
        let to_name = if duct_from_override.iter().any(|(d, _)| d == id) {
            "outside"
        } else {
            raw.get("to").and_then(Value::as_str).unwrap_or("")
        };
        let a = index_of(from_name)?;
        let b = index_of(to_name)?;
        if a.is_none() && b.is_none() {
            return Err(format!("duct {id} runs from outside to outside."));
        }
        if raw.get("kind").and_then(Value::as_str) == Some("passive_radiator") {
            if b.is_some() {
                return Err(format!(
                    "duct {id}: a passive radiator must open to the outside."
                ));
            }
            let pr = passive_radiator_of(raw).map_err(|e| format!("duct {id}: {e}"))?;
            let chamber =
                a.ok_or_else(|| format!("duct {id}: a passive radiator must leave a chamber."))?;
            outside_count += 1;
            edges.push(Edge {
                a: Some(chamber),
                b: None,
                section: Section {
                    round: true,
                    start: vec![(4.0 * pr.sd_m2 / std::f64::consts::PI).sqrt()],
                    end: vec![(4.0 * pr.sd_m2 / std::f64::consts::PI).sqrt()],
                    tube_m: 0.0,
                },
                length: 0.0,
                s0: 0.0,
                s1: 1.0,
                outside_slot: Some(outside_count - 1),
                slices: None,
                passive: Some(pr),
            });
            continue;
        }
        let section = Section::of(raw).map_err(|e| format!("duct {id}: {e}"))?;
        let length = path_length(&path(raw));
        if length <= 0.0 {
            return Err(format!("duct {id} has no path."));
        }
        let tap = duct_taps
            .iter()
            .find(|(d, _)| d == id)
            .map(|(_, t)| *t)
            .unwrap_or(0.0);
        // Offset tap: open section from tap → mouth, plus a rigid closed stub tap → closed end.
        if duct_from_override.iter().any(|(d, _)| d == id) && tap > 1e-9 {
            let couple = a.expect("duct mount remaps to a coupling chamber");
            let open_len = length * (1.0 - tap);
            outside_count += 1;
            edges.push(Edge {
                a: Some(couple),
                b: None,
                section: section.clone(),
                length: open_len,
                s0: tap,
                s1: 1.0,
                outside_slot: Some(outside_count - 1),
                slices: None,
                passive: None,
            });
            rigid_stubs.push(RigidStub {
                at: couple,
                section,
                length: length * tap,
                s0: 0.0,
                s1: tap,
            });
            continue;
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
            s0: 0.0,
            s1: 1.0,
            outside_slot,
            slices: None,
            passive: None,
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
            s0: 0.0,
            s1: 1.0,
            outside_slot: Some(outside_count - 1),
            slices: Some(merged),
            passive: None,
        });
    }
    // Lumped merges closed-stub air into the coupling volume; the detailed model carries that
    // air in the rigid stub branch, so strip it from the chamber volumes here.
    let mut wave_volumes = volumes_m3.to_vec();
    for stub in &rigid_stubs {
        let full = stub.length / (stub.s1 - stub.s0).max(1e-15);
        let stub_vol = stub.section.volume(full, stub.s0, stub.s1, 0.0);
        if stub.at < wave_volumes.len() {
            wave_volumes[stub.at] = (wave_volumes[stub.at] - stub_vol).max(1e-9);
        }
    }
    let mut order = Vec::new();
    let mut seen = vec![false; names.len()];
    let mut used = vec![false; edges.len()];
    #[allow(clippy::too_many_arguments)]
    fn chamber_node(
        at: usize,
        volumes: &[f64],
        fills: &[Option<f64>],
        edges: &[Edge],
        stubs: &[RigidStub],
        seen: &mut Vec<bool>,
        used: &mut Vec<bool>,
        order: &mut Vec<usize>,
    ) -> Result<AcousticNode, String> {
        seen[at] = true;
        let mut branches = vec![AcousticNode::Chamber {
            volume_m3: volumes[at],
            slot: at,
            fill_sigma: fills.get(at).copied().flatten(),
        }];
        for stub in stubs.iter().filter(|s| s.at == at) {
            let a_near = stub.section.area(stub.s1, 0.0); // tap end toward the chamber
            let a_far = stub.section.area(stub.s0, 0.0); // closed end
            let r_near = (a_near / std::f64::consts::PI).sqrt();
            branches.push(AcousticNode::Duct {
                length_m: stub.length,
                area_in_m2: a_near,
                area_out_m2: a_far,
                added_length_m: 0.849 * r_near,
                load: Box::new(AcousticNode::Rigid),
            });
        }
        for (index, edge) in edges.iter().enumerate() {
            if used[index] || (edge.a != Some(at) && edge.b != Some(at)) {
                continue;
            }
            used[index] = true;
            if let Some(pr) = edge.passive {
                order.push(edge.outside_slot.unwrap_or(0));
                branches.push(AcousticNode::PassiveRadiator {
                    mech: pr,
                    radiation: None,
                });
                continue;
            }
            // Orient the duct from this chamber.
            let forward = edge.a == Some(at);
            let far = if forward { edge.b } else { edge.a };
            let (s_near, s_far) = if forward {
                (edge.s0, edge.s1)
            } else {
                (edge.s1, edge.s0)
            };
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
                        return Err("loop".into());
                    }
                    (
                        chamber_node(other, volumes, fills, edges, stubs, seen, used, order)?,
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
    let mut notes = Vec::new();
    let mut wave_fills: Vec<Option<f64>> = fill_sigma.to_vec();
    wave_fills.resize(wave_volumes.len(), None);
    let body = match chamber_node(
        driver,
        &wave_volumes,
        &wave_fills,
        &edges,
        &rigid_stubs,
        &mut seen,
        &mut used,
        &mut order,
    ) {
        Ok(root) => WaveguideBody::Tree(root),
        Err(err) if err == "loop" => {
            // Same ducts as Y-parameter branches on a chamber nodal matrix (README 5.4).
            // Rigid stubs become ducts into a near-zero volume chamber (closed terminus).
            let mut mesh_volumes = wave_volumes.clone();
            let mut mesh_fills = wave_fills.clone();
            let mut mesh_edges = Vec::with_capacity(edges.len() + rigid_stubs.len());
            let mut mesh_order = Vec::new();
            for stub in &rigid_stubs {
                let stub_node = mesh_volumes.len();
                mesh_volumes.push(1e-12);
                mesh_fills.push(None);
                let a_near = stub.section.area(stub.s1, 0.0);
                let a_far = stub.section.area(stub.s0, 0.0);
                let r_near = (a_near / std::f64::consts::PI).sqrt();
                mesh_edges.push(MeshDuct {
                    from: stub.at,
                    to: Some(stub_node),
                    length_m: stub.length,
                    area_in_m2: a_near,
                    area_out_m2: a_far,
                    added_length_m: 0.849 * r_near,
                    slices: None,
                    outside_slot: None,
                    mouth_area_m2: a_far,
                    passive: None,
                });
            }
            for edge in &edges {
                let (a_near, a_far) = (
                    edge.section.area(edge.s0, 0.0),
                    edge.section.area(edge.s1, 0.0),
                );
                let r_near = (a_near / std::f64::consts::PI).sqrt();
                let r_far = (a_far / std::f64::consts::PI).sqrt();
                let (from, to, added, mouth) = match (edge.a, edge.b) {
                    (Some(a), Some(b)) => (a, Some(b), 0.849 * r_near + 0.849 * r_far, a_far),
                    (Some(a), None) => (a, None, 0.849 * r_near, a_far),
                    (None, Some(b)) => (b, None, 0.849 * r_far, a_near),
                    (None, None) => continue,
                };
                if let Some(slot) = edge.outside_slot {
                    mesh_order.push(slot);
                }
                mesh_edges.push(MeshDuct {
                    from,
                    to,
                    length_m: edge.length,
                    area_in_m2: if edge.a.is_some() { a_near } else { a_far },
                    area_out_m2: if edge.a.is_some() { a_far } else { a_near },
                    added_length_m: if edge.slices.is_some() || edge.passive.is_some() {
                        0.0
                    } else {
                        added
                    },
                    slices: edge.slices.clone(),
                    outside_slot: edge.outside_slot,
                    mouth_area_m2: mouth,
                    passive: edge.passive,
                });
            }
            mesh_order.sort_unstable();
            order = mesh_order;
            notes.push(
                "Its chambers and ducts form a loop: the detailed model solves them as a mesh \
                 of duct transmission lines, not a tree."
                    .into(),
            );
            WaveguideBody::Mesh(WaveguideMesh {
                volumes_m3: mesh_volumes,
                fill_sigma: mesh_fills,
                edges: mesh_edges,
                driver,
            })
        }
        Err(err) => return Err(err),
    };
    if fill_sigma.iter().any(|s| s.is_some()) {
        notes.push(
            "A chamber has damping fill: the detailed model uses Delany–Bazley (Allard–Champoux) \
             complex bulk modulus from the fill's flow resistivity; the full 3D model uses a \
             Rayleigh update with the same resistivity, so the two will not match bin-for-bin."
                .into(),
        );
        let band = fill_sigma.iter().flatten().any(|&sigma| {
            seatlock_acoustics::delany_bazley_out_of_band(sigma, 20.0)
                || seatlock_acoustics::delany_bazley_out_of_band(sigma, 20_000.0)
        });
        if band {
            notes.push(
                "Delany–Bazley's quoted band is 0.01 < f/σ < 1 (σ in SI); some frequencies in \
                 the audio band sit outside that band for this fill."
                    .into(),
            );
        }
    }
    if matches!(body, WaveguideBody::Tree(_)) && used.iter().any(|u| !u) {
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
        body,
        opening_order: order,
        notes,
    })
}

/// Mechanical parameters of a passive-radiator duct from its catalog id or inline published
/// fields (README §5).
pub fn passive_radiator_of(raw: &Value) -> Result<PassiveRadiatorMech, String> {
    let published = if let Some(id) = raw
        .get("passive_radiator_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        catalog::passive_radiator(id)
            .ok_or_else(|| format!("passive radiator {id} is not in the catalog."))?
            .get("published")
            .cloned()
            .ok_or_else(|| format!("passive radiator {id} has no published parameters."))?
    } else if let Some(p) = raw.get("published") {
        p.clone()
    } else {
        return Err(
            "name a catalog passive_radiator_id, or give published Mms, Cms, Rms, and Sd.".into(),
        );
    };
    let num = |key: &str| -> Result<f64, String> {
        published
            .get(key)
            .and_then(Value::as_f64)
            .filter(|v| *v > 0.0)
            .ok_or_else(|| format!("published.{key} must be a positive number."))
    };
    let mms_kg = num("mms_kg")?;
    let cms_m_n = num("cms_m_per_n")?;
    let sd_m2 = num("sd_m2")?;
    let rms_ns_m = published
        .get("rms_ns_per_m")
        .and_then(Value::as_f64)
        .filter(|v| *v >= 0.0)
        .or_else(|| {
            let qms = published.get("qms").and_then(Value::as_f64)?;
            let fs = published.get("fs_hz").and_then(Value::as_f64)?;
            if qms > 0.0 && fs > 0.0 {
                Some(2.0 * std::f64::consts::PI * fs * mms_kg / qms)
            } else {
                None
            }
        })
        .ok_or("published.rms_ns_per_m (or qms and fs_hz) is required.")?;
    Ok(PassiveRadiatorMech {
        mms_kg,
        cms_m_n,
        rms_ns_m,
        sd_m2,
    })
}
