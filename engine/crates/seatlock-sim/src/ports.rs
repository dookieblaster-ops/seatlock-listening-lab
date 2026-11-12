//! Ports and ducts in the builder (README sections 6.2 and 6.4). A duct to `outside` is
//! bored through the wall it leaves by and printed as a tube or a slot reaching back into
//! its chamber; a duct between two chambers is printed as a tube through the partition.
//!
//! A duct's path is straight runs joined at corners (bends). A port's last run leaves one of
//! the shape's faces square. A round section may bend any way; a slot bends only within one
//! plane. Either may taper. Anything else is still simulated, but the print file is refused
//! with a reason.

use seatlock_core::registry::OuterShape;
use seatlock_core::DesignFile;
use serde_json::{json, Value};

use crate::chambers::{air_intervals, Chamber, Section};

/// Wall of a printed port tube unless the duct gives `wall_m`. A lab default, not a catalog
/// number (STATUS decisions).
pub const DEFAULT_PORT_WALL_M: f64 = 0.003;

/// What one printed duct adds to the cabinet.
pub struct PrintedPort {
    pub id: String,
    /// Added to the cabinet before anything is cut.
    pub shell: Value,
    /// Cut after the shells are added.
    pub bore: Value,
    /// Plastic the duct adds: its tube where it runs through air, less its bore where it
    /// runs through a wall or partition.
    pub solid_m3: f64,
    /// Air the tube's outside takes from the chambers.
    pub displaced_m3: f64,
    /// For a port: where it opens outside, its direction, half-width, and face.
    pub opening_m: [f64; 3],
    pub outward: [f64; 3],
    pub half_width_m: f64,
    pub surface: String,
    pub to_outside: bool,
    /// Each straight run's center line, past its corners, with the tube's half-width.
    pub lines: Vec<([f64; 3], [f64; 3], f64)>,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add_scaled(a: [f64; 3], d: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn point(value: &Value) -> Option<[f64; 3]> {
    let items = value.as_array()?;
    Some([
        items.first()?.as_f64()?,
        items.get(1)?.as_f64()?,
        items.get(2)?.as_f64()?,
    ])
}

/// Newell's normal of a flat outline.
fn face_normal(corners: &[[f64; 3]]) -> [f64; 3] {
    let mut n = [0.0; 3];
    for i in 0..corners.len() {
        let (p, q) = (corners[i], corners[(i + 1) % corners.len()]);
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    let length = dot(n, n).sqrt().max(1e-12);
    [n[0] / length, n[1] / length, n[2] / length]
}

/// Every duct in the design, in design order, as a printed duct or the reason it cannot be
/// printed. `chambers` is empty when the whole inside is one chamber.
pub fn printed_ports(
    design: &DesignFile,
    shape: &dyn OuterShape,
    chambers: &[Chamber],
) -> Vec<Result<PrintedPort, String>> {
    let mut out = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        let from = duct.get("from").and_then(Value::as_str).unwrap_or("");
        let to = duct.get("to").and_then(Value::as_str).unwrap_or("");
        let id = duct
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("port")
            .to_string();
        let built = if from == "outside" || to == "outside" {
            one_port(duct, from == "outside", shape, chambers)
        } else {
            inner_duct(duct, from, to, chambers)
        };
        out.push(built.map_err(|e| format!("Port {id}: {e}")));
    }
    out
}

/// One straight run of a duct's path. `s0` and `s1` are the fractions of the whole path at
/// its ends; `ext0` and `ext1` lengthen it past a corner by half the section there, so the
/// runs meet without a notch (README 6.2: a corner in the path is a bend).
#[derive(Clone, Debug)]
pub struct Seg {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub dir: [f64; 3],
    pub length: f64,
    pub s0: f64,
    pub s1: f64,
    pub ext0: f64,
    pub ext1: f64,
    /// Across the section: a slot's width runs along `right` and its height along `up`.
    pub right: [f64; 3],
    pub up: [f64; 3],
}

/// A duct's whole path, from its inside end (outward for a port).
pub struct Run {
    pub id: String,
    pub segs: Vec<Seg>,
    pub length: f64,
    pub section: Section,
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot(a, a).sqrt().max(1e-15);
    [a[0] / length, a[1] / length, a[2] / length]
}

impl Run {
    pub fn inner(&self) -> [f64; 3] {
        self.segs[0].a
    }
    pub fn outer(&self) -> [f64; 3] {
        self.segs[self.segs.len() - 1].b
    }
    pub fn last(&self) -> &Seg {
        &self.segs[self.segs.len() - 1]
    }
    pub fn bends(&self) -> usize {
        self.segs.len() - 1
    }
}

/// A duct's path as straight runs, checked for what the builder can make: a round section
/// bends any way; a slot bends only within one plane, so its height turns the corners and
/// its width stays across the fold.
pub fn run(duct: &Value, reversed: bool) -> Result<Run, String> {
    let id = duct
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("port")
        .to_string();
    let mut path: Vec<[f64; 3]> = duct
        .get("path_m")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(point).collect())
        .unwrap_or_default();
    let mut section = Section::of(duct)?;
    if reversed {
        path.reverse();
        std::mem::swap(&mut section.start, &mut section.end);
    }
    if path.len() < 2 {
        return Err("its path needs at least two points.".into());
    }
    let mut dirs = Vec::new();
    let mut lengths = Vec::new();
    for pair in path.windows(2) {
        let along = sub(pair[1], pair[0]);
        let length = dot(along, along).sqrt();
        if length <= 1e-9 {
            return Err("two points of its path are the same.".into());
        }
        dirs.push([along[0] / length, along[1] / length, along[2] / length]);
        lengths.push(length);
    }
    for pair in dirs.windows(2) {
        if dot(pair[0], pair[1]) < -0.999 {
            return Err("its path doubles straight back on itself.".into());
        }
    }
    let total: f64 = lengths.iter().sum();
    // The fold plane of a bent slot.
    let mut normal: Option<[f64; 3]> = None;
    if !section.round && dirs.len() > 1 {
        for pair in dirs.windows(2) {
            let n = cross(pair[0], pair[1]);
            if dot(n, n).sqrt() < 1e-9 {
                continue;
            }
            let n = unit(n);
            match normal {
                None => normal = Some(n),
                Some(m) if dot(m, n).abs() > 0.9999 => {}
                Some(_) => {
                    return Err(
                        "a slot that bends must stay in one plane; this one does not.".into(),
                    )
                }
            }
        }
        if let Some(n) = normal {
            if dirs.iter().any(|d| dot(*d, n).abs() > 1e-6) {
                return Err("a slot that bends must stay in one plane; this one does not.".into());
            }
        }
    }
    let half_turn = |s: f64| -> f64 {
        // Half the section's size in the plane of the turn.
        let size: Vec<f64> = section
            .start
            .iter()
            .zip(&section.end)
            .map(|(a, b)| a + (b - a) * s)
            .collect();
        if section.round {
            size[0] / 2.0
        } else {
            size[1] / 2.0
        }
    };
    let mut segs = Vec::new();
    let mut done = 0.0;
    for (index, dir) in dirs.iter().enumerate() {
        let (right, up) = match normal {
            Some(n) => (n, unit(cross(*dir, n))),
            None => seatlock_geom::prism::face_axes(*dir),
        };
        let (s0, s1) = (done / total, (done + lengths[index]) / total);
        let ext0 = if index > 0 { half_turn(s0) } else { 0.0 };
        let ext1 = if index + 1 < dirs.len() {
            half_turn(s1)
        } else {
            0.0
        };
        segs.push(Seg {
            a: path[index],
            b: path[index + 1],
            dir: *dir,
            length: lengths[index],
            s0,
            s1,
            ext0,
            ext1,
            right,
            up,
        });
        done += lengths[index];
    }
    Ok(Run {
        id,
        segs,
        length: total,
        section,
    })
}

/// Plastic and displaced air of a duct. Along its first `inside` meters it is a tube where
/// it passes through chamber air; everywhere else (partitions, the solid around a folded
/// line, and the outer wall after `inside`) its bore takes plastic out. Measured along the
/// center line, which is exact for a slot's square corners.
fn volumes(d: &Run, inside: f64, chambers: &[Chamber]) -> (f64, f64) {
    let s = &d.section;
    let (mut solid, mut displaced, mut covered) = (0.0, 0.0, 0.0);
    let mut start = 0.0;
    for seg in &d.segs {
        for (t0, t1) in air_intervals(seg.a, seg.b, chambers) {
            let (g0, g1) = (start + t0, (start + t1).min(inside));
            if g1 <= g0 {
                continue;
            }
            let (s0, s1) = (g0 / d.length, g1 / d.length);
            let outside = s.volume(d.length, s0, s1, s.tube_m);
            let bore = s.volume(d.length, s0, s1, 0.0);
            solid += outside - bore;
            displaced += outside;
            covered += bore;
        }
        start += seg.length;
    }
    solid -= s.volume(d.length, 0.0, 1.0, 0.0) - covered;
    (solid, displaced)
}

fn size_at(section: &Section, s: f64, grow: f64) -> Vec<f64> {
    section
        .start
        .iter()
        .zip(&section.end)
        .map(|(a, b)| a + (b - a) * s + 2.0 * grow)
        .collect()
}

/// One straight run as a solid, grown by `grow`, lengthened by `extra` at the path's two
/// open ends (a cutter must pass cleanly through the faces there).
fn seg_csg(d: &Run, seg: &Seg, grow: f64, extra: f64) -> Value {
    let first = seg.s0 <= 0.0;
    let last = seg.s1 >= 1.0 - 1e-12;
    let back = seg.ext0 + if first { extra } else { 0.0 };
    let ahead = seg.ext1 + if last { extra } else { 0.0 };
    let base = add_scaled(seg.a, seg.dir, -back);
    let length = seg.length + back + ahead;
    let (s0, s1) = (
        size_at(&d.section, seg.s0, grow),
        size_at(&d.section, seg.s1, grow),
    );
    // Sizes carried straight on past the run's ends, so a taper keeps its slope.
    let slope: Vec<f64> = s0
        .iter()
        .zip(&s1)
        .map(|(a, b)| (b - a) / seg.length)
        .collect();
    let at_base: Vec<f64> = s0.iter().zip(&slope).map(|(a, k)| a - k * back).collect();
    let at_end: Vec<f64> = s1.iter().zip(&slope).map(|(a, k)| a + k * ahead).collect();
    if d.section.round {
        return json!({ "op": "cylinder", "base_m": base, "axis": seg.dir, "length_m": length,
                       "radius_m": at_base[0] / 2.0, "radius_end_m": at_end[0] / 2.0,
                       "segments": 96 });
    }
    if (at_base[0] - at_end[0]).abs() < 1e-12 && (at_base[1] - at_end[1]).abs() < 1e-12 {
        let (hw, hh) = (at_base[0] / 2.0, at_base[1] / 2.0);
        return json!({ "op": "prism", "outline_m": [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]],
                       "origin_m": base, "u": seg.right, "v": seg.up, "w": seg.dir,
                       "length_m": length });
    }
    // A tapered slot: the hull of its two end rectangles.
    let far = add_scaled(base, seg.dir, length);
    let mut points = Vec::new();
    for (at, size) in [(base, &at_base), (far, &at_end)] {
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            points.push(add_scaled(
                add_scaled(at, seg.right, a * size[0] / 2.0),
                seg.up,
                b * size[1] / 2.0,
            ));
        }
    }
    seatlock_geom::edged::csg_hull(points)
}

fn csg(d: &Run, grow: f64, extra: f64) -> Value {
    let parts: Vec<Value> = d
        .segs
        .iter()
        .map(|seg| seg_csg(d, seg, grow, extra))
        .collect();
    if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        seatlock_geom::csg_union(parts)
    }
}

/// Half the widest size across the tube's outside at fraction `s`.
fn half_width_at(d: &Run, s: f64) -> f64 {
    let t = d.section.tube_m;
    let size = size_at(&d.section, s, t);
    if d.section.round {
        size[0] / 2.0
    } else {
        0.5 * (size[0] * size[0] + size[1] * size[1]).sqrt()
    }
}

fn half_width(d: &Run) -> f64 {
    half_width_at(d, 0.0).max(half_width_at(d, 1.0))
}

/// Each run's center line with the half-width of its tube, to keep pins away from it.
fn lines(d: &Run) -> Vec<([f64; 3], [f64; 3], f64)> {
    let reach = half_width(d);
    d.segs
        .iter()
        .map(|seg| {
            (
                add_scaled(seg.a, seg.dir, -seg.ext0),
                add_scaled(seg.b, seg.dir, seg.ext1),
                reach,
            )
        })
        .collect()
}

type InAir<'a> = &'a dyn Fn([f64; 3]) -> Result<bool, String>;

/// True when the tube's outside fits in the air at the path's inside end: the corners of a
/// slot, or four points round a round tube.
fn fits(d: &Run, in_air: InAir) -> Result<bool, String> {
    let seg = &d.segs[0];
    let size = size_at(&d.section, 0.0, d.section.tube_m);
    let (a, b) = if d.section.round {
        (size[0] / 2.0, size[0] / 2.0)
    } else {
        (size[0] / 2.0, size[1] / 2.0)
    };
    let points: &[(f64, f64)] = if d.section.round {
        &[(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
    } else {
        &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
    };
    for (u, v) in points {
        let p = add_scaled(
            add_scaled(seg.a, seg.right, u * a * 0.999),
            seg.up,
            v * b * 0.999,
        );
        if !in_air(p)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn one_port(
    duct: &Value,
    reversed: bool,
    shape: &dyn OuterShape,
    chambers: &[Chamber],
) -> Result<PrintedPort, String> {
    let d = run(duct, reversed)?;
    let last = d.last().clone();
    let surface = shape
        .surface_facing(last.dir)
        .ok_or("its outside end does not leave through a face of the cabinet.")?;
    let corners = shape.surface_outline(&surface)?;
    let mut normal = face_normal(&corners);
    if dot(normal, last.dir) < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    if dot(normal, last.dir) < 0.9998 {
        return Err(format!(
            "it leaves the {surface} face at a slant; only a port square to its face is printed."
        ));
    }
    if dot(sub(d.outer(), corners[0]), normal).abs() > 1e-6 {
        return Err(format!(
            "its outside end is not on the {surface} face. A port's path runs from inside the \
             chamber to the outside surface."
        ));
    }
    let wall = shape.panel_thickness_m(&surface)?;
    if last.length - last.ext0 <= wall {
        return Err("its last straight run is no longer than the wall it goes through.".into());
    }
    let inside = d.length - wall;
    let in_air = |p: [f64; 3]| -> Result<bool, String> {
        Ok(shape.contains_air(p)?
            && (chambers.is_empty() || chambers.iter().any(|chamber| chamber.contains(p))))
    };
    if !in_air(d.inner())? {
        return Err("its inside end is not in a chamber's air.".into());
    }
    for seg in &d.segs[1..] {
        if !shape.contains_air(seg.a)? {
            return Err("a corner of its path is outside the cabinet's inside.".into());
        }
    }
    if !fits(&d, &in_air)? {
        return Err("its tube does not fit inside the chamber.".into());
    }
    let (solid, displaced) = volumes(&d, inside, chambers);
    Ok(PrintedPort {
        id: d.id.clone(),
        shell: csg(&d, d.section.tube_m, 0.0),
        bore: csg(&d, 0.0, 0.0005),
        solid_m3: solid,
        displaced_m3: displaced,
        opening_m: d.outer(),
        outward: last.dir,
        half_width_m: half_width_at(&d, 1.0),
        surface,
        to_outside: true,
        lines: lines(&d),
    })
}

fn inner_duct(
    duct: &Value,
    from: &str,
    to: &str,
    chambers: &[Chamber],
) -> Result<PrintedPort, String> {
    let d = run(duct, false)?;
    let find = |id: &str| chambers.iter().find(|chamber| chamber.id == id);
    let (Some(a), Some(b)) = (find(from), find(to)) else {
        return Err(format!(
            "it joins {from} and {to}, which are not both chambers."
        ));
    };
    if !a.contains(d.inner()) || !b.contains(d.outer()) {
        return Err(format!("its ends are not inside chambers {from} and {to}."));
    }
    let in_a = |p: [f64; 3]| Ok(a.contains(p));
    if !fits(&d, &in_a)? {
        return Err("its tube does not fit inside the chambers at its ends.".into());
    }
    let reversed = run(duct, true)?;
    let in_b = |p: [f64; 3]| Ok(b.contains(p));
    if !fits(&reversed, &in_b)? {
        return Err("its tube does not fit inside the chambers at its ends.".into());
    }
    let (solid, displaced) = volumes(&d, d.length, chambers);
    let last = d.last().clone();
    Ok(PrintedPort {
        id: d.id.clone(),
        shell: csg(&d, d.section.tube_m, 0.0),
        bore: csg(&d, 0.0, 0.0005),
        solid_m3: solid,
        displaced_m3: displaced,
        opening_m: d.outer(),
        outward: last.dir,
        half_width_m: half_width(&d),
        surface: String::new(),
        to_outside: false,
        lines: lines(&d),
    })
}

/// A straight duct's bore for the 3D solver's grid: the air inside it, and its printed tube.
#[derive(Clone, Debug)]
pub struct Bore {
    pub id: String,
    /// Inside end, and the unit direction toward the other end (outward for a port).
    pub inner: [f64; 3],
    pub dir: [f64; 3],
    pub length: f64,
    pub round: bool,
    /// Section sizes at the inside end and the other end: `[diameter]` or `[width, height]`.
    pub start: Vec<f64>,
    pub end: Vec<f64>,
    pub tube_m: f64,
    pub to_outside: bool,
    /// How far this run reaches past a corner at each end (section sizes hold there).
    pub ext0: f64,
    pub ext1: f64,
    pub right: [f64; 3],
    pub up: [f64; 3],
}

impl Bore {
    /// Section size at distance `t` from the inside end, grown by `grow` on every side.
    pub fn size_at(&self, t: f64, grow: f64) -> Vec<f64> {
        let span = (self.length - self.ext0 - self.ext1).max(1e-12);
        let s = ((t - self.ext0) / span).clamp(0.0, 1.0);
        self.start
            .iter()
            .zip(&self.end)
            .map(|(a, b)| a + (b - a) * s + 2.0 * grow)
            .collect()
    }

    /// Bore area at distance `t` from the inside end.
    pub fn area_at(&self, t: f64) -> f64 {
        let d = self.size_at(t, 0.0);
        if self.round {
            std::f64::consts::PI * d[0] * d[0] / 4.0
        } else {
            d[0] * d[1]
        }
    }

    /// `(t, normalized distance from the axis)`: 1 at the section's edge, grown by `grow`.
    pub fn locate(&self, p: [f64; 3], grow: f64) -> (f64, f64) {
        let rel = sub(p, self.inner);
        let t = dot(rel, self.dir);
        let across = add_scaled(rel, self.dir, -t);
        let size = self.size_at(t, grow);
        if self.round {
            (t, dot(across, across).sqrt() / (size[0] / 2.0))
        } else {
            let u = dot(across, self.right).abs() / (size[0] / 2.0);
            let v = dot(across, self.up).abs() / (size[1] / 2.0);
            (t, u.max(v))
        }
    }

    /// The largest size across the opening, for the exterior block.
    pub fn width_m(&self) -> f64 {
        self.start
            .iter()
            .chain(&self.end)
            .fold(0.0_f64, |a, b| a.max(*b))
    }

    pub fn mouth_m(&self) -> [f64; 3] {
        add_scaled(self.inner, self.dir, self.length)
    }
}

/// Every duct's straight runs as bores, in design order, each with the index of its duct.
/// Only the last run of a duct to the outside opens to the outside.
pub fn bores(design: &DesignFile) -> Vec<Result<Vec<Bore>, String>> {
    let mut out = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        let from = duct.get("from").and_then(Value::as_str).unwrap_or("");
        let to = duct.get("to").and_then(Value::as_str).unwrap_or("");
        let outside = from == "outside" || to == "outside";
        let built = run(duct, from == "outside").map(|d| {
            let count = d.segs.len();
            d.segs
                .iter()
                .enumerate()
                .map(|(index, seg)| Bore {
                    id: d.id.clone(),
                    inner: add_scaled(seg.a, seg.dir, -seg.ext0),
                    dir: seg.dir,
                    length: seg.length + seg.ext0 + seg.ext1,
                    round: d.section.round,
                    start: size_at(&d.section, seg.s0, 0.0),
                    end: size_at(&d.section, seg.s1, 0.0),
                    tube_m: d.section.tube_m,
                    to_outside: outside && index + 1 == count,
                    ext0: seg.ext0,
                    ext1: seg.ext1,
                    right: seg.right,
                    up: seg.up,
                })
                .collect()
        });
        out.push(built);
    }
    out
}
