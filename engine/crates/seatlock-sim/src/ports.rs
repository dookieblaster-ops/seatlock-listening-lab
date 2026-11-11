//! Ports in the builder (README sections 6.2 and 6.4): a duct to `outside` is bored through
//! the wall it leaves by and printed as a tube or a slot reaching back into the chamber.
//!
//! What is printed today: a straight duct whose outside end lies on one of the shape's
//! faces and leaves it square. A round section may taper; a slot keeps one size. Anything
//! else is still simulated, but the print file is refused with a reason.

use seatlock_core::registry::OuterShape;
use seatlock_core::DesignFile;
use serde_json::{json, Value};

/// Wall of a printed port tube unless the duct gives `wall_m`. A lab default, not a catalog
/// number (STATUS decisions).
pub const DEFAULT_PORT_WALL_M: f64 = 0.003;

/// What one printed port adds to the cabinet.
pub struct PrintedPort {
    pub id: String,
    /// Added to the cabinet before anything is cut.
    pub shell: Value,
    /// Cut after the shells are added.
    pub bore: Value,
    /// Plastic the port adds, after its bore through the wall is taken out.
    pub solid_m3: f64,
    /// Air the tube's outside takes from the chamber.
    pub displaced_m3: f64,
    /// Where it opens on the outside, its outward direction, and its largest half-width.
    pub opening_m: [f64; 3],
    pub outward: [f64; 3],
    pub half_width_m: f64,
    pub surface: String,
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

fn frustum_m3(length: f64, r0: f64, r1: f64) -> f64 {
    std::f64::consts::PI * length / 3.0 * (r0 * r0 + r0 * r1 + r1 * r1)
}

/// The ducts that leave the cabinet, in the order the design lists them, each as a printed
/// port or the reason it cannot be printed.
pub fn printed_ports(
    design: &DesignFile,
    shape: &dyn OuterShape,
) -> Vec<Result<PrintedPort, String>> {
    let mut out = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        let from = duct.get("from").and_then(Value::as_str).unwrap_or("");
        let to = duct.get("to").and_then(Value::as_str).unwrap_or("");
        if from != "outside" && to != "outside" {
            continue;
        }
        let id = duct
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("port")
            .to_string();
        out.push(one_port(duct, from == "outside", shape).map_err(|e| format!("Port {id}: {e}")));
    }
    out
}

fn one_port(duct: &Value, reversed: bool, shape: &dyn OuterShape) -> Result<PrintedPort, String> {
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
    let section = duct.get("section").cloned().unwrap_or(Value::Null);
    let size = |key: &str| -> Vec<f64> {
        section
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default()
    };
    let (mut start_size, mut end_size) = (size("start_m"), size("end_m"));
    if end_size.is_empty() {
        end_size = start_size.clone();
    }
    if reversed {
        path.reverse();
        std::mem::swap(&mut start_size, &mut end_size);
    }
    if path.len() != 2 {
        return Err(
            "only a straight port is printed so far; this one is simulated, and the print file \
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
    let dir = [along[0] / length, along[1] / length, along[2] / length];
    let surface = shape
        .surface_facing(dir)
        .ok_or("its outside end does not leave through a face of the cabinet.")?;
    let corners = shape.surface_outline(&surface)?;
    let mut normal = face_normal(&corners);
    if dot(normal, dir) < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    if dot(normal, dir) < 0.9998 {
        return Err(format!(
            "it leaves the {surface} face at a slant; only a port square to its face is printed."
        ));
    }
    if dot(sub(outer, corners[0]), normal).abs() > 1e-6 {
        return Err(format!(
            "its outside end is not on the {surface} face. A port's path runs from inside the \
             chamber to the outside surface."
        ));
    }
    let wall = shape.panel_thickness_m(&surface)?;
    let inside = length - wall;
    if inside <= 0.0 {
        return Err("it is no longer than the wall it goes through.".into());
    }
    if !shape.contains_air(inner)? {
        return Err("its inside end is not in the chamber's air.".into());
    }
    let tube = duct
        .get("wall_m")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_PORT_WALL_M);
    let kind = section
        .get("shape")
        .and_then(Value::as_str)
        .unwrap_or("round");
    let extra = 0.0005;
    let bore_from = add_scaled(inner, dir, -extra);
    let (shell, bore, solid, displaced, half_width) = match kind {
        "round" => {
            let (r0, r1) = match (start_size.first(), end_size.first()) {
                (Some(a), Some(b)) if *a > 0.0 && *b > 0.0 => (a / 2.0, b / 2.0),
                _ => return Err("its round section has no diameter.".into()),
            };
            let at_wall = r0 + (r1 - r0) * inside / length;
            let shell = cylinder(inner, dir, length, r0 + tube, r1 + tube);
            let slope = (r1 - r0) / length;
            let bore = cylinder(
                bore_from,
                dir,
                length + 2.0 * extra,
                r0 - slope * extra,
                r1 + slope * extra,
            );
            let outside = frustum_m3(inside, r0 + tube, at_wall + tube);
            let solid = outside - frustum_m3(inside, r0, at_wall) - frustum_m3(wall, at_wall, r1);
            (shell, bore, solid, outside, r0.max(r1) + tube)
        }
        "rect" | "slot" => {
            let (w, h) = match (start_size.as_slice(), end_size.as_slice()) {
                ([w0, h0], [w1, h1]) if w0 == w1 && h0 == h1 && *w0 > 0.0 && *h0 > 0.0 => {
                    (*w0, *h0)
                }
                ([_, _], [_, _]) => {
                    return Err(
                        "a tapered slot is not printed yet; the print file is refused.".into(),
                    )
                }
                _ => return Err("its slot section needs a width and a height.".into()),
            };
            // The slot's width runs across the face and its height up it.
            let (right, up) = seatlock_geom::prism::face_axes(dir);
            let shell = slot(
                inner,
                right,
                up,
                dir,
                length,
                w + 2.0 * tube,
                h + 2.0 * tube,
            );
            let bore = slot(bore_from, right, up, dir, length + 2.0 * extra, w, h);
            let outside = (w + 2.0 * tube) * (h + 2.0 * tube) * inside;
            let solid = outside - w * h * inside - w * h * wall;
            let half = 0.5 * ((w + 2.0 * tube).powi(2) + (h + 2.0 * tube).powi(2)).sqrt();
            (shell, bore, solid, outside, half)
        }
        other => return Err(format!("a {other} section is not printed.")),
    };
    // The tube's outside must stay in the chamber: check its far corners at the inside end.
    let (right, up) = seatlock_geom::prism::face_axes(dir);
    let reach = half_width * 0.999;
    for (a, b) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let probe = add_scaled(add_scaled(inner, right, a * reach), up, b * reach);
        if !shape.contains_air(probe)? {
            return Err("its tube does not fit inside the chamber.".into());
        }
    }
    Ok(PrintedPort {
        id,
        shell,
        bore,
        solid_m3: solid,
        displaced_m3: displaced,
        opening_m: outer,
        outward: dir,
        half_width_m: half_width,
        surface,
    })
}

fn cylinder(base: [f64; 3], axis: [f64; 3], length: f64, r0: f64, r1: f64) -> Value {
    json!({ "op": "cylinder", "base_m": base, "axis": axis, "length_m": length,
            "radius_m": r0, "radius_end_m": r1, "segments": 96 })
}

fn slot(
    base: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    axis: [f64; 3],
    length: f64,
    width: f64,
    height: f64,
) -> Value {
    let (hw, hh) = (width / 2.0, height / 2.0);
    json!({ "op": "prism", "outline_m": [[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh]],
            "origin_m": base, "u": right, "v": up, "w": axis, "length_m": length })
}
