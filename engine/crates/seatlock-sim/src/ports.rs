//! Ports and ducts in the builder (README sections 6.2 and 6.4). A duct to `outside` is
//! bored through the wall it leaves by and printed as a tube or a slot reaching back into
//! its chamber; a duct between two chambers is printed as a tube through the partition.
//!
//! What is printed today: straight ducts. A port's outside end lies on one of the shape's
//! faces and leaves it square. A round section may taper; a slot keeps one size. Anything
//! else is still simulated, but the print file is refused with a reason.

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

struct Straight {
    id: String,
    inner: [f64; 3],
    outer: [f64; 3],
    dir: [f64; 3],
    length: f64,
    section: Section,
}

fn straight(duct: &Value, reversed: bool) -> Result<Straight, String> {
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
    if path.len() != 2 {
        return Err(
            "only a straight duct is printed so far; this one is simulated, and the print file \
             is refused."
                .into(),
        );
    }
    let (inner, outer) = (path[0], path[1]);
    let along = sub(outer, inner);
    let length = dot(along, along).sqrt();
    if length <= 0.0 {
        return Err("its path has no length.".into());
    }
    if !section.round && section.start != section.end {
        return Err("a tapered slot is not printed yet; the print file is refused.".into());
    }
    Ok(Straight {
        id,
        inner,
        outer,
        dir: [along[0] / length, along[1] / length, along[2] / length],
        length,
        section,
    })
}

/// Plastic and displaced air of a straight duct. Along its first `run` meters it is a tube
/// where it passes through chamber air; everywhere else (partitions, and the outer wall
/// after `run`) its bore takes plastic out.
fn volumes(d: &Straight, run: f64, chambers: &[Chamber]) -> (f64, f64) {
    let s = &d.section;
    let end = add_scaled(d.inner, d.dir, run);
    let (mut solid, mut displaced, mut covered) = (0.0, 0.0, 0.0);
    for (t0, t1) in air_intervals(d.inner, end, chambers) {
        let (s0, s1) = (t0 / d.length, t1 / d.length);
        let outside = s.volume(d.length, s0, s1, s.tube_m);
        let bore = s.volume(d.length, s0, s1, 0.0);
        solid += outside - bore;
        displaced += outside;
        covered += bore;
    }
    solid -= s.volume(d.length, 0.0, 1.0, 0.0) - covered;
    (solid, displaced)
}

fn csg(d: &Straight, grow: f64, extra: f64) -> Value {
    let s = &d.section;
    let base = add_scaled(d.inner, d.dir, -extra);
    let length = d.length + 2.0 * extra;
    if s.round {
        let slope = (s.end[0] - s.start[0]) / 2.0 / d.length;
        json!({ "op": "cylinder", "base_m": base, "axis": d.dir, "length_m": length,
                "radius_m": s.start[0] / 2.0 + grow - slope * extra,
                "radius_end_m": s.end[0] / 2.0 + grow + slope * extra, "segments": 96 })
    } else {
        // The slot's width runs across the face and its height up it.
        let (right, up) = seatlock_geom::prism::face_axes(d.dir);
        let (hw, hh) = (s.start[0] / 2.0 + grow, s.start[1] / 2.0 + grow);
        json!({ "op": "prism", "outline_m": [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]],
                "origin_m": base, "u": right, "v": up, "w": d.dir, "length_m": length })
    }
}

fn half_width(d: &Straight) -> f64 {
    let s = &d.section;
    let t = s.tube_m;
    if s.round {
        s.start[0].max(s.end[0]) / 2.0 + t
    } else {
        0.5 * ((s.start[0] + 2.0 * t).powi(2) + (s.start[1] + 2.0 * t).powi(2)).sqrt()
    }
}

type InAir<'a> = &'a dyn Fn([f64; 3]) -> Result<bool, String>;

/// True when the tube's outside fits in the air at `at`: four far points across it.
fn fits(d: &Straight, at: [f64; 3], in_air: InAir) -> Result<bool, String> {
    let (right, up) = seatlock_geom::prism::face_axes(d.dir);
    let reach = half_width(d) * 0.999;
    for (a, b) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        if !in_air(add_scaled(add_scaled(at, right, a * reach), up, b * reach))? {
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
    let d = straight(duct, reversed)?;
    let surface = shape
        .surface_facing(d.dir)
        .ok_or("its outside end does not leave through a face of the cabinet.")?;
    let corners = shape.surface_outline(&surface)?;
    let mut normal = face_normal(&corners);
    if dot(normal, d.dir) < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    if dot(normal, d.dir) < 0.9998 {
        return Err(format!(
            "it leaves the {surface} face at a slant; only a port square to its face is printed."
        ));
    }
    if dot(sub(d.outer, corners[0]), normal).abs() > 1e-6 {
        return Err(format!(
            "its outside end is not on the {surface} face. A port's path runs from inside the \
             chamber to the outside surface."
        ));
    }
    let wall = shape.panel_thickness_m(&surface)?;
    let inside = d.length - wall;
    if inside <= 0.0 {
        return Err("it is no longer than the wall it goes through.".into());
    }
    let in_air = |p: [f64; 3]| -> Result<bool, String> {
        Ok(shape.contains_air(p)?
            && (chambers.is_empty() || chambers.iter().any(|chamber| chamber.contains(p))))
    };
    if !in_air(d.inner)? {
        return Err("its inside end is not in a chamber's air.".into());
    }
    if !fits(&d, d.inner, &in_air)? {
        return Err("its tube does not fit inside the chamber.".into());
    }
    let (solid, displaced) = volumes(&d, inside, chambers);
    Ok(PrintedPort {
        id: d.id.clone(),
        shell: csg(&d, d.section.tube_m, 0.0),
        bore: csg(&d, 0.0, 0.0005),
        solid_m3: solid,
        displaced_m3: displaced,
        opening_m: d.outer,
        outward: d.dir,
        half_width_m: half_width(&d),
        surface,
        to_outside: true,
    })
}

fn inner_duct(
    duct: &Value,
    from: &str,
    to: &str,
    chambers: &[Chamber],
) -> Result<PrintedPort, String> {
    let d = straight(duct, false)?;
    let find = |id: &str| chambers.iter().find(|chamber| chamber.id == id);
    let (Some(a), Some(b)) = (find(from), find(to)) else {
        return Err(format!(
            "it joins {from} and {to}, which are not both chambers."
        ));
    };
    if !a.contains(d.inner) || !b.contains(d.outer) {
        return Err(format!("its ends are not inside chambers {from} and {to}."));
    }
    let in_a = |p: [f64; 3]| Ok(a.contains(p));
    let in_b = |p: [f64; 3]| Ok(b.contains(p));
    if !fits(&d, d.inner, &in_a)? || !fits(&d, d.outer, &in_b)? {
        return Err("its tube does not fit inside the chambers at its ends.".into());
    }
    let (solid, displaced) = volumes(&d, d.length, chambers);
    Ok(PrintedPort {
        id: d.id.clone(),
        shell: csg(&d, d.section.tube_m, 0.0),
        bore: csg(&d, 0.0, 0.0005),
        solid_m3: solid,
        displaced_m3: displaced,
        opening_m: d.outer,
        outward: d.dir,
        half_width_m: half_width(&d),
        surface: String::new(),
        to_outside: false,
    })
}
