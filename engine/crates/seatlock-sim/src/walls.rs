//! The cabinet's walls as sound sources (README section 5.11): which walls, how big, of what
//! layers, and where their elements sit.

use num_complex::Complex64;
use seatlock_acoustics::laminate::{ASSUMED_LOSS_FACTOR, ASSUMED_POISSON};
use seatlock_acoustics::wall::{WallLayer, WallPanel, WallStack};
use seatlock_core::catalog;
use seatlock_core::design::DesignFile;
use seatlock_core::registry::{OuterShape, SourcePatch};
use serde_json::Value;

/// Outer shapes whose walls are printed plastic of the design's `wall_m`.
const PRINTED: [&str; 6] = ["box", "rounded", "chamfered", "sloped", "prism", "revolved"];

/// Printed shapes plus imported mesh cabinets (walls measured from the solid).
fn walls_modeled(kind: &str) -> bool {
    PRINTED.contains(&kind) || kind == "mesh"
}

pub struct PlacedWall {
    pub surface: String,
    /// The chamber whose pressure pushes this wall (0 when the inside is one chamber).
    pub chamber: usize,
    pub panel: WallPanel,
    /// One patch per element of `panel`, in the panel's element order.
    pub patches: Vec<SourcePatch>,
}

/// A plate between two chambers, pushed by their pressure difference (README 5.11). It does
/// not radiate outside; it only couples the air network.
pub struct Partition {
    pub chamber_a: usize,
    pub chamber_b: usize,
    pub panel: WallPanel,
    pub thickness_m: f64,
}

pub struct CabinetWalls {
    pub walls: Vec<PlacedWall>,
    pub partitions: Vec<Partition>,
    /// Lines for the fidelity label.
    pub notes: Vec<String>,
}

impl CabinetWalls {
    /// Outward volume velocity of all outer walls together per pascal inside.
    pub fn admittance(&self, freq_hz: f64) -> Complex64 {
        self.walls.iter().map(|w| w.panel.admittance(freq_hz)).sum()
    }

    /// Outward volume velocity of every outer-wall element per pascal inside, walls in order.
    pub fn element_admittances(&self, freq_hz: f64) -> Vec<Complex64> {
        self.walls
            .iter()
            .flat_map(|w| w.panel.element_admittances(freq_hz))
            .collect()
    }

    pub fn patches(&self) -> Vec<SourcePatch> {
        self.walls
            .iter()
            .flat_map(|w| w.patches.iter().copied())
            .collect()
    }

    /// Outward volume velocity of one chamber's outer walls per pascal in that chamber.
    pub fn chamber_admittance(&self, chamber: usize, freq_hz: f64) -> Complex64 {
        self.walls
            .iter()
            .filter(|w| w.chamber == chamber)
            .map(|w| w.panel.admittance(freq_hz))
            .sum()
    }

    /// Partition links `(chamber_a, chamber_b, Y)` for the lumped network: volume velocity
    /// from `a` into `b` per pascal of `(P_a - P_b)`.
    pub fn partition_admittances(&self, freq_hz: f64) -> Vec<(usize, usize, Complex64)> {
        self.partitions
            .iter()
            .map(|p| (p.chamber_a, p.chamber_b, p.panel.admittance(freq_hz)))
            .collect()
    }

    /// For every element, in the order of [`CabinetWalls::patches`], its chamber.
    pub fn element_chambers(&self) -> Vec<usize> {
        self.walls
            .iter()
            .flat_map(|w| std::iter::repeat_n(w.chamber, w.patches.len()))
            .collect()
    }

    /// The lowest wall or partition resonance, in Hz.
    pub fn first_resonance_hz(&self) -> f64 {
        self.walls
            .iter()
            .map(|w| w.panel.first_resonance_hz())
            .chain(self.partitions.iter().map(|p| p.panel.first_resonance_hz()))
            .fold(f64::INFINITY, f64::min)
    }
}

fn number(record: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| record.get(*key).and_then(Value::as_f64))
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn name_of(record: &Value, id: &str) -> String {
    record
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string()
}

/// A layer from a print material record, `infill` of it solid.
fn printed_layer(
    id: &str,
    thickness_m: f64,
    infill: f64,
    notes: &mut Vec<String>,
) -> Result<WallLayer, String> {
    let record = catalog::material(id)
        .ok_or_else(|| format!("print material {id} is not in the catalog"))?;
    let name = name_of(&record, id);
    let density = number(&record, &["density_kg_m3"])
        .ok_or_else(|| format!("print material {name} has no density"))?;
    let youngs = number(
        &record,
        &["youngs_modulus_xy_pa", "youngs_pa", "bending_modulus_xy_pa"],
    );
    let poisson = number(&record, &["poisson"]).unwrap_or_else(|| {
        notes.push(format!(
            "{name}: Poisson's ratio is not published; the lab assumed {ASSUMED_POISSON}."
        ));
        ASSUMED_POISSON
    });
    let loss_factor = record
        .get("loss_factor")
        .and_then(Value::as_f64)
        .unwrap_or_else(|| {
            notes.push(format!(
                "{name}: the damping (loss factor) is not published; the lab assumed \
                 {ASSUMED_LOSS_FACTOR}, so wall resonance heights are uncertain."
            ));
            ASSUMED_LOSS_FACTOR
        });
    let youngs = match youngs {
        Some(e) => e * infill,
        None => {
            notes.push(format!(
                "{name}: its stiffness is not published, so the walls are modeled as two skins \
                 with {name} as a loose mass between them."
            ));
            0.0
        }
    };
    Ok(WallLayer {
        name,
        thickness_m,
        youngs_pa: youngs,
        poisson,
        density_kg_m3: density * infill,
        loss_factor,
    })
}

/// The wall's layers from the inside face out, from `build.material` and `build.wall`.
pub fn wall_layers(
    design: &DesignFile,
    wall_m: f64,
) -> Result<(Vec<WallLayer>, Vec<String>), String> {
    let mut notes = Vec::new();
    let infill = design.build.infill_fraction.clamp(0.0, 1.0);
    if infill < 1.0 {
        notes.push(format!(
            "The walls are printed at {:.0} % infill; their stiffness and mass are taken as that \
             share of solid plastic, an estimate.",
            infill * 100.0
        ));
    }
    let skin_material = design.build.material.as_str();
    let wall = design.build.extra.get("wall").and_then(Value::as_object);
    let kind = wall
        .and_then(|w| w.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("solid");
    if kind == "solid" {
        let layer = printed_layer(skin_material, wall_m, infill, &mut notes)?;
        if layer.youngs_pa <= 0.0 {
            return Err(format!(
                "the print material {} publishes no stiffness",
                layer.name
            ));
        }
        return Ok((vec![layer], notes));
    }
    let wall = wall.ok_or("the wall build is not readable")?;
    let skin = wall.get("skin_m").and_then(Value::as_f64).unwrap_or(0.0016);
    if !(skin > 0.0 && 2.0 * skin < wall_m) {
        return Err("the wall build's skins leave no room for a core".into());
    }
    let core_m = wall_m - 2.0 * skin;
    let skin_layer = printed_layer(skin_material, skin, infill, &mut notes)?;
    if skin_layer.youngs_pa <= 0.0 {
        return Err(format!(
            "the skin material {} publishes no stiffness",
            skin_layer.name
        ));
    }
    let core = match kind {
        "filled" => {
            let id = wall.get("fill").and_then(Value::as_str).unwrap_or("");
            let record =
                catalog::fill(id).ok_or_else(|| format!("wall fill {id} is not in the catalog"))?;
            let name = name_of(&record, id);
            let density = number(&record, &["density_kg_m3"])
                .ok_or_else(|| format!("wall fill {name} has no density"))?;
            let youngs = number(&record, &["youngs_pa"]).unwrap_or(0.0);
            let loss_factor = record
                .get("loss_factor")
                .and_then(Value::as_f64)
                .unwrap_or_else(|| {
                    notes.push(format!(
                        "{name}: its damping is not published, so the lab counts none from it."
                    ));
                    0.0
                });
            let poisson = if youngs > 0.0 {
                number(&record, &["poisson"]).unwrap_or_else(|| {
                    notes.push(format!(
                        "{name}: Poisson's ratio is not published; the lab assumed \
                         {ASSUMED_POISSON}."
                    ));
                    ASSUMED_POISSON
                })
            } else {
                0.0
            };
            if youngs > 0.0 {
                notes.push(format!(
                    "{name} sets bonded to both skins and stiffens the wall with them."
                ));
            } else {
                notes.push(format!(
                    "{name} is loose: it adds mass and its own damping in proportion to its \
                     share of the mass, and no stiffness, so the skins bend apart."
                ));
            }
            WallLayer {
                name,
                thickness_m: core_m,
                youngs_pa: youngs,
                poisson,
                density_kg_m3: density,
                loss_factor,
            }
        }
        "two_material" => {
            let id = wall.get("core").and_then(Value::as_str).unwrap_or("");
            let mut layer = printed_layer(id, core_m, infill, &mut notes)?;
            if layer.youngs_pa > 0.0 {
                notes.push(format!(
                    "The {} core is bonded between the skins; a soft core damps the wall as it \
                     shears (constrained-layer damping, Ross-Kerwin-Ungar).",
                    layer.name
                ));
            } else {
                // Its loss acts on its share of the mass, as for a loose fill.
                layer.poisson = 0.0;
            }
            layer
        }
        other => return Err(format!("unknown wall build {other}")),
    };
    Ok((vec![skin_layer.clone(), core, skin_layer], notes))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt().max(1e-300);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// One rectangular plate of a wall after braces have split it: size and centre in the wall's
/// along/across frame, relative to the wall's centre.
struct WallTile {
    width: f64,
    height: f64,
    mid_u: f64,
    mid_v: f64,
}

/// Split a wall plate where braces reach it. A brace that runs most of the way across the
/// plate is a shelf or stud; a small end-on contact (a cross-brace into this wall) still cuts
/// the plate's longer side through the contact. Returns the tiles and how many braces cut.
#[allow(clippy::too_many_arguments)]
fn wall_tiles(
    span_w: f64,
    span_h: f64,
    braces: &[crate::solids::Brace],
    centre: [f64; 3],
    outward: [f64; 3],
    along: [f64; 3],
    across: [f64; 3],
    wall_m: f64,
) -> (Vec<WallTile>, usize) {
    let half_w = span_w / 2.0;
    let half_h = span_h / 2.0;
    let mut u_cuts = Vec::new();
    let mut v_cuts = Vec::new();
    let mut used = 0usize;
    for brace in braces {
        let mut out_lo = f64::MAX;
        let mut out_hi = f64::MIN;
        let mut u_lo = f64::MAX;
        let mut u_hi = f64::MIN;
        let mut v_lo = f64::MAX;
        let mut v_hi = f64::MIN;
        for &x in &[brace.min_m[0], brace.max_m[0]] {
            for &y in &[brace.min_m[1], brace.max_m[1]] {
                for &z in &[brace.min_m[2], brace.max_m[2]] {
                    let rel = sub([x, y, z], centre);
                    let o = dot(rel, outward);
                    out_lo = out_lo.min(o);
                    out_hi = out_hi.max(o);
                    let u = dot(rel, along);
                    let v = dot(rel, across);
                    u_lo = u_lo.min(u);
                    u_hi = u_hi.max(u);
                    v_lo = v_lo.min(v);
                    v_hi = v_hi.max(v);
                }
            }
        }
        // Outer face ~0 on this frame; inner face ~-wall_m. The brace touches when its
        // outward-most face sits near the inner face.
        let touches = (out_hi + wall_m).abs() < 0.5 * wall_m
            || (out_hi > -1.5 * wall_m && out_hi < 0.25 * wall_m && out_lo < -0.5 * wall_m);
        if !touches {
            continue;
        }
        if u_hi < -half_w || u_lo > half_w || v_hi < -half_h || v_lo > half_h {
            continue;
        }
        let u_mid = ((u_lo + u_hi) / 2.0).clamp(-0.95 * half_w, 0.95 * half_w);
        let v_mid = ((v_lo + v_hi) / 2.0).clamp(-0.95 * half_h, 0.95 * half_h);
        let du = u_hi - u_lo;
        let dv = v_hi - v_lo;
        let cut_v =
            du >= 0.5 * span_w || (du < 0.5 * span_w && dv < 0.5 * span_h && span_h >= span_w);
        let cut_u = !cut_v
            && (dv >= 0.5 * span_h || (du < 0.5 * span_w && dv < 0.5 * span_h && span_w > span_h));
        // Keep a cut only when both resulting plates still have a usable span (not a
        // sliver against one edge). A cut through the middle is the usual case.
        if cut_v && (half_h - v_mid.abs()) > 0.05 * span_h {
            v_cuts.push(v_mid);
            used += 1;
        } else if cut_u && (half_w - u_mid.abs()) > 0.05 * span_w {
            u_cuts.push(u_mid);
            used += 1;
        }
    }
    u_cuts.sort_by(|a, b| a.total_cmp(b));
    v_cuts.sort_by(|a, b| a.total_cmp(b));
    u_cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    v_cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let mut u_edges = vec![-half_w];
    u_edges.extend(u_cuts);
    u_edges.push(half_w);
    let mut v_edges = vec![-half_h];
    v_edges.extend(v_cuts);
    v_edges.push(half_h);
    let mut tiles = Vec::new();
    for ui in 0..u_edges.len() - 1 {
        for vi in 0..v_edges.len() - 1 {
            let width = u_edges[ui + 1] - u_edges[ui];
            let height = v_edges[vi + 1] - v_edges[vi];
            if width <= wall_m || height <= wall_m {
                continue;
            }
            tiles.push(WallTile {
                width,
                height,
                mid_u: (u_edges[ui] + u_edges[ui + 1]) / 2.0,
                mid_v: (v_edges[vi] + v_edges[vi + 1]) / 2.0,
            });
        }
    }
    if tiles.is_empty() {
        tiles.push(WallTile {
            width: span_w,
            height: span_h,
            mid_u: 0.0,
            mid_v: 0.0,
        });
    }
    (tiles, used)
}

/// A figure from a stock record: the number itself, or the middle of a published range.
fn stock_figure(
    record: &Value,
    key: &str,
    range_key: &str,
    what: &str,
    name: &str,
    notes: &mut Vec<String>,
) -> Option<f64> {
    if let Some(v) = number(record, &[key]) {
        return Some(v);
    }
    let items = record.get(range_key)?.as_array()?;
    let (a, b) = (items.first()?.as_f64()?, items.get(1)?.as_f64()?);
    let middle = (a + b) / 2.0;
    if !(middle.is_finite() && middle > 0.0) {
        return None;
    }
    notes.push(format!(
        "{name}: its {what} is published as a range, {a} to {b}; the lab takes the middle."
    ));
    Some(middle)
}

/// A sheet box's wall: one layer of its stock sheet.
fn sheet_layer(design: &DesignFile, notes: &mut Vec<String>) -> Result<WallLayer, String> {
    let id = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("stock_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let record =
        catalog::stock(id).ok_or_else(|| format!("sheet stock {id} is not in the catalog"))?;
    let name = record
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(id)
        .to_string();
    let thickness = number(&record, &["thickness_m"]).ok_or("the sheet has no thickness")?;
    let youngs = stock_figure(
        &record,
        "youngs_modulus_pa",
        "youngs_modulus_range_pa",
        "stiffness (Pa)",
        &name,
        notes,
    );
    let density = stock_figure(
        &record,
        "density_kg_m3",
        "density_range_kg_m3",
        "density (kg/m3)",
        &name,
        notes,
    );
    let missing: Vec<&str> = [("stiffness", youngs), ("density", density)]
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(n, _)| *n)
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "the sheet {name} has no published {} in the catalog",
            missing.join(" or ")
        ));
    }
    let poisson = number(&record, &["poisson"]).unwrap_or_else(|| {
        notes.push(format!(
            "{name}: Poisson's ratio is not published; the lab assumed {ASSUMED_POISSON}."
        ));
        ASSUMED_POISSON
    });
    let loss_factor = record
        .get("loss_factor")
        .and_then(Value::as_f64)
        .unwrap_or_else(|| {
            notes.push(format!(
                "{name}: the damping (loss factor) is not published; the lab assumed \
                 {ASSUMED_LOSS_FACTOR}, so wall resonance heights are uncertain."
            ));
            ASSUMED_LOSS_FACTOR
        });
    Ok(WallLayer {
        name,
        thickness_m: thickness,
        youngs_pa: youngs.unwrap_or(0.0),
        poisson,
        density_kg_m3: density.unwrap_or(0.0),
        loss_factor,
    })
}

/// Axis-aligned bounds of the air inside an imported cabinet, found by sampling
/// [`OuterShape::contains_air`], then marching each face out to the air–solid interface.
fn air_aabb(outer: &dyn OuterShape) -> Result<([f64; 3], [f64; 3]), String> {
    let size = outer.outer_size_m()?;
    let step = (size[0].min(size[1]).min(size[2]) / 50.0).clamp(0.002, 0.008);
    let n = [
        ((size[0] / step).ceil() as usize).max(2),
        ((size[1] / step).ceil() as usize).max(2),
        ((size[2] / step).ceil() as usize).max(2),
    ];
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    let mut any = false;
    for ix in 0..=n[0] {
        for iy in 0..=n[1] {
            for iz in 0..=n[2] {
                let p = [
                    size[0] * ix as f64 / n[0] as f64,
                    size[1] * iy as f64 / n[1] as f64,
                    size[2] * iz as f64 / n[2] as f64,
                ];
                if !outer.contains_air(p).unwrap_or(false) {
                    continue;
                }
                any = true;
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    if !any {
        return Err("no enclosed air was found inside the imported cabinet".into());
    }
    // March each face outward from the sample bound to the last air point.
    let march = 2.5e-4;
    let mid = [
        0.5 * (lo[0] + hi[0]),
        0.5 * (lo[1] + hi[1]),
        0.5 * (lo[2] + hi[2]),
    ];
    for axis in 0..3 {
        let mut t = lo[axis];
        while t > 0.0 {
            let mut p = mid;
            p[axis] = t - march;
            if !outer.contains_air(p).unwrap_or(false) {
                break;
            }
            t -= march;
        }
        lo[axis] = t;
        t = hi[axis];
        while t < size[axis] {
            let mut p = mid;
            p[axis] = t + march;
            if !outer.contains_air(p).unwrap_or(false) {
                break;
            }
            t += march;
        }
        hi[axis] = t;
        if hi[axis] <= lo[axis] + march {
            return Err("the imported cabinet's air is too thin to model as walls".into());
        }
    }
    Ok((lo, hi))
}

/// The cabinet's flat walls, or the reason they are left rigid.
pub fn cabinet_walls(design: &DesignFile, outer: &dyn OuterShape) -> Result<CabinetWalls, String> {
    let kind = design.speaker.enclosure.outer.kind.as_str();
    // An imported mesh with one air space: treat that air's bounds as a single chamber and
    // measure each outer wall through the solid (README 5.11).
    if kind == "mesh" {
        let (lo, hi) = air_aabb(outer)?;
        let chamber = crate::chambers::Chamber {
            id: "main".into(),
            min_m: lo,
            max_m: hi,
            plastic_m3: 0.0,
            fill: None,
            fill_sigma: None,
            region: None,
        };
        let mut cabinet = chamber_walls(design, outer, std::slice::from_ref(&chamber))?;
        cabinet.notes.insert(
            0,
            "Imported cabinet: flat outer walls are measured through the solid from the air; \
             curved or slanted skin is left still; wall layers follow the design's print \
             material and wall build."
                .into(),
        );
        return Ok(cabinet);
    }
    // A tube's walls are its two printed end caps; each spans the pipe's inside, as a square
    // of the same area. The curved pipe is held still.
    let mut cap_span = None;
    let (wall_m, (layers, mut notes)) = match kind {
        _ if PRINTED.contains(&kind) => {
            let wall_m = design
                .speaker
                .enclosure
                .outer
                .fields
                .get("wall_m")
                .and_then(Value::as_f64)
                .filter(|w| *w > 0.0)
                .ok_or("the cabinet gives no wall thickness")?;
            (wall_m, wall_layers(design, wall_m)?)
        }
        "sheet_box" => {
            let mut notes = vec![
                "The sheet box's panels are butt-jointed; each is taken as supported along its \
                 edges."
                    .to_string(),
            ];
            let layer = sheet_layer(design, &mut notes)?;
            (layer.thickness_m, (vec![layer], notes))
        }
        "tube" => {
            let cap = outer.panel_thickness_m("front")?;
            let size = outer.outer_size_m()?;
            let length = size[1] - 2.0 * cap;
            let inside = (outer.interior_volume_m3()? / (std::f64::consts::PI * length)).sqrt();
            cap_span = Some(inside * std::f64::consts::PI.sqrt());
            let (layers, mut notes) = wall_layers(design, cap)?;
            notes.push(
                "Only the tube's printed end caps vibrate, each taken as a square of the pipe's \
                 inside area; the curved pipe is held still."
                    .into(),
            );
            (cap, (layers, notes))
        }
        _ => {
            return Err(format!(
                "walls of a {kind} cabinet are not modeled yet (printed cabinets, imported \
                 mesh cabinets, sheet boxes, and tubes' end caps are)"
            ))
        }
    };
    let stack = WallStack::from_layers(&layers)?;
    let braces = crate::solids::solids(design)
        .map(|p| p.braces)
        .unwrap_or_default();
    let size = outer.outer_size_m()?;
    let middle = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let mut walls = Vec::new();
    let mut approximated = false;
    let mut braces_used = 0usize;
    // A wall that carries a sound source of its own (a panel speaker) is left still: the
    // source covers it.
    let covered: Vec<&str> = design
        .speaker
        .radiators
        .iter()
        .filter(|r| r.kind != "cone")
        .filter_map(|r| r.fields.get("mount").and_then(Value::as_str))
        .filter_map(|id| {
            design
                .speaker
                .enclosure
                .mounts
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.surface.as_str())
        })
        .collect();
    for surface in outer.surfaces() {
        if covered.contains(&surface.as_str()) {
            notes.push(format!(
                "The {surface} wall carries a panel speaker and is held still."
            ));
            continue;
        }
        let Ok(corners) = outer.surface_outline(&surface) else {
            continue;
        };
        if corners.len() < 3 {
            continue;
        }
        // Newell's normal, the area, and the corners' mean.
        let mut normal = [0.0; 3];
        let mut centre = [0.0; 3];
        for (i, a) in corners.iter().enumerate() {
            let b = corners[(i + 1) % corners.len()];
            normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
            normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
            normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
            for axis in 0..3 {
                centre[axis] += a[axis] / corners.len() as f64;
            }
        }
        let area = dot(normal, normal).sqrt() / 2.0;
        let mut outward = unit(normal);
        if dot(outward, sub(centre, middle)) < 0.0 {
            outward = [-outward[0], -outward[1], -outward[2]];
        }
        let along = unit(sub(corners[1], corners[0]));
        let across = cross(outward, along);
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for corner in &corners {
            let rel = sub(*corner, centre);
            for (axis, direction) in [along, across].iter().enumerate() {
                let x = dot(rel, *direction);
                lo[axis] = lo[axis].min(x);
                hi[axis] = hi[axis].max(x);
            }
        }
        let (mut width, mut height) = (hi[0] - lo[0], hi[1] - lo[1]);
        let mid_offset = [(hi[0] + lo[0]) / 2.0, (hi[1] + lo[1]) / 2.0];
        if (width * height - area).abs() > 1e-3 * area {
            // Not a rectangle: a rectangle of the same area and proportions.
            let scale = (area / (width * height)).sqrt();
            width *= scale;
            height *= scale;
            approximated = true;
        }
        // The plate spans between the middles of the walls along its edges.
        let (span_w, span_h) = match cap_span {
            Some(side) => (side, side),
            None => (width - wall_m, height - wall_m),
        };
        if span_w <= wall_m || span_h <= wall_m {
            continue;
        }
        let (tiles, used) = wall_tiles(
            span_w, span_h, &braces, centre, outward, along, across, wall_m,
        );
        braces_used += used;
        for tile in tiles {
            let panel = WallPanel::new(tile.width, tile.height, &stack)?;
            let element_area = panel.element_area_m2();
            let patches = panel
                .element_centers_m()
                .iter()
                .map(|[x, y]| {
                    let (x, y) = (
                        x + mid_offset[0] + tile.mid_u,
                        y + mid_offset[1] + tile.mid_v,
                    );
                    SourcePatch::skin(
                        [
                            centre[0] + x * along[0] + y * across[0],
                            centre[1] + x * along[1] + y * across[1],
                            centre[2] + x * along[2] + y * across[2],
                        ],
                        outward,
                        element_area,
                    )
                })
                .collect();
            walls.push(PlacedWall {
                surface: surface.clone(),
                chamber: 0,
                panel,
                patches,
            });
        }
    }
    if walls.is_empty() {
        return Err("the cabinet has no flat wall the model can take".into());
    }
    let first = walls
        .iter()
        .map(|w| w.panel.first_resonance_hz())
        .fold(f64::INFINITY, f64::min);
    let resolved = walls
        .iter()
        .map(|w| w.panel.resolved_up_to_hz)
        .fold(f64::INFINITY, f64::min);
    let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
    notes.insert(
        0,
        format!(
            "Walls vibrate: {} flat walls of {} ({:.1} kg/m2), each a plate with supported \
             edges pushed by the cabinet's mean pressure; the lowest wall resonance is {first:.0} \
             Hz, and wall resonances above {resolved:.0} Hz are left out.",
            walls.len(),
            names.join(" / "),
            stack.mass_per_area
        ),
    );
    if braces_used > 0 {
        notes.push(format!(
            "{braces_used} brace contact(s) split wall plates into smaller supported plates, \
             raising their resonances; braces take air and print as plastic."
        ));
    } else if !braces.is_empty() {
        notes.push(
            "The design has braces that do not reach a vibrating wall face, so they take air \
             and print but do not split a wall."
                .into(),
        );
    }
    notes.push(
        "Wall vibration leaves out the air's load on the walls, standing waves' pressure \
         pattern on them, the driver's frame shaking the baffle, and holes in the walls; \
         curved skin does not vibrate in this model."
            .into(),
    );
    if approximated {
        notes
            .push("Walls that are not rectangles are taken as rectangles of the same area.".into());
    }
    Ok(CabinetWalls {
        walls,
        partitions: Vec::new(),
        notes,
    })
}

/// Surfaces that carry a sound source other than a cone (a panel speaker): their walls are
/// held still, because the source covers them.
fn covered_surfaces(design: &DesignFile) -> Vec<String> {
    design
        .speaker
        .radiators
        .iter()
        .filter(|r| r.kind != "cone")
        .filter_map(|r| r.fields.get("mount").and_then(Value::as_str))
        .filter_map(|id| {
            design
                .speaker
                .enclosure
                .mounts
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.surface.clone())
        })
        .collect()
}

enum FaceBeyond {
    Outside(f64),
    /// Neighbor chamber index and the plastic thickness to its face.
    Partition(usize, f64),
    Held,
}

/// What the ray from a chamber face hits within `limit_m`: the outside, another chamber
/// (a partition), or neither (thicker plastic / open air that is not a chamber).
/// Outside and partition distances are the solid thickness only (air before the wall is
/// skipped), so an inset air bound still yields the real wall thickness.
fn face_beyond(
    outer: &dyn OuterShape,
    chambers: &[crate::chambers::Chamber],
    own: usize,
    from: [f64; 3],
    direction: [f64; 3],
    limit_m: f64,
) -> FaceBeyond {
    let step = 2.5e-4;
    let mut t = 0.0;
    let mut solid_start: Option<f64> = None;
    while t < limit_m {
        t += step;
        let p = [
            from[0] + t * direction[0],
            from[1] + t * direction[1],
            from[2] + t * direction[2],
        ];
        if !outer.contains_cabinet(p).unwrap_or(true) {
            return match solid_start {
                Some(t0) => FaceBeyond::Outside((t - t0).max(step)),
                None => FaceBeyond::Held, // left the cabinet without crossing solid
            };
        }
        if let Some((i, _)) = chambers
            .iter()
            .enumerate()
            .find(|(i, c)| *i != own && c.contains(p))
        {
            return match solid_start {
                Some(t0) => FaceBeyond::Partition(i, (t - t0).max(step)),
                None => FaceBeyond::Partition(i, t),
            };
        }
        // Air ahead of the wall does not count toward thickness.
        let in_air = outer.contains_air(p).unwrap_or(false);
        if in_air {
            solid_start = None;
        } else if solid_start.is_none() {
            solid_start = Some(t);
        }
    }
    FaceBeyond::Held
}

/// The walls of a cabinet divided into chambers (README 5.11): each face of a chamber that
/// lies against the cabinet's outer wall is a plate over that face, pushed by that chamber's
/// pressure. Faces against another chamber are partitions (two-sided plates); faces against
/// thicker plastic are held still. Printed cabinets and imported mesh cabinets both use this.
pub fn chamber_walls(
    design: &DesignFile,
    outer: &dyn OuterShape,
    chambers: &[crate::chambers::Chamber],
) -> Result<CabinetWalls, String> {
    let kind = design.speaker.enclosure.outer.kind.as_str();
    if !walls_modeled(kind) {
        return Err(format!(
            "walls of a {kind} cabinet are not modeled yet (printed and imported mesh cabinets)"
        ));
    }
    let field_wall_m = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("wall_m")
        .and_then(Value::as_f64)
        .filter(|w| *w > 0.0);
    // Imported mesh has no wall_m field: thickness is measured per face. A printed cabinet
    // keeps one stack from its wall_m. The probe limit is 1.5 walls, or 50 mm when unknown.
    let probe_limit = field_wall_m.map(|w| 1.5 * w).unwrap_or(0.05) + 1e-6;
    let (shared_layers, mut notes) = match field_wall_m {
        Some(wall_m) => wall_layers(design, wall_m)?,
        None => (Vec::new(), Vec::new()),
    };
    let shared_stack = if shared_layers.is_empty() {
        None
    } else {
        Some(WallStack::from_layers(&shared_layers)?)
    };
    let braces = crate::solids::solids(design)
        .map(|p| p.braces)
        .unwrap_or_default();
    let covered = covered_surfaces(design);
    let infill = design.build.infill_fraction.clamp(0.0, 1.0);
    let mut walls = Vec::new();
    let mut partitions = Vec::new();
    let mut held = 0usize;
    let mut braces_used = 0usize;
    let mut layer_names: Vec<String> = shared_layers.iter().map(|l| l.name.clone()).collect();
    let mut mass_per_area = shared_stack
        .as_ref()
        .map(|s| s.mass_per_area)
        .unwrap_or(0.0);
    let mut measured_notes_done = shared_stack.is_some();
    let mut thickness_min = f64::INFINITY;
    let mut thickness_max = 0.0f64;
    // Partition pairs already made (lower chamber index, higher), so each plate is once.
    let mut seen_partitions = std::collections::HashSet::<(usize, usize)>::new();
    for (index, chamber) in chambers.iter().enumerate() {
        for axis in 0..3 {
            for high in [false, true] {
                let mut outward = [0.0; 3];
                outward[axis] = if high { 1.0 } else { -1.0 };
                let (u_axis, v_axis) = match axis {
                    0 => (1, 2),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let mut centre: [f64; 3] =
                    std::array::from_fn(|i| (chamber.min_m[i] + chamber.max_m[i]) / 2.0);
                centre[axis] = if high {
                    chamber.max_m[axis]
                } else {
                    chamber.min_m[axis]
                };
                let width = chamber.max_m[u_axis] - chamber.min_m[u_axis];
                let height = chamber.max_m[v_axis] - chamber.min_m[v_axis];
                // Probe the middle and four points halfway to the corners.
                let mut probes = Vec::new();
                for (du, dv) in [
                    (0.0, 0.0),
                    (-0.25, -0.25),
                    (0.25, -0.25),
                    (-0.25, 0.25),
                    (0.25, 0.25),
                ] {
                    let mut p = centre;
                    p[u_axis] += du * width;
                    p[v_axis] += dv * height;
                    probes.push(face_beyond(outer, chambers, index, p, outward, probe_limit));
                }
                let all_outside = probes.iter().all(|p| matches!(p, FaceBeyond::Outside(_)));
                let all_partition = probes
                    .iter()
                    .all(|p| matches!(p, FaceBeyond::Partition(_, _)));
                if all_outside {
                    let FaceBeyond::Outside(thickness) = probes[0] else {
                        unreachable!()
                    };
                    // All five probes should agree on thickness within a wall; else hold still.
                    let same_thickness = probes.iter().all(|p| match p {
                        FaceBeyond::Outside(t) => {
                            (t - thickness).abs() < 0.25 * thickness.max(1e-4)
                        }
                        _ => false,
                    });
                    if !same_thickness || thickness <= 0.0 {
                        held += 1;
                        continue;
                    }
                    let surface = outer
                        .surface_facing(outward)
                        .unwrap_or_else(|| format!("chamber {} side", chamber.id));
                    if covered.iter().any(|c| c == &surface) {
                        notes.push(format!(
                            "The {surface} wall carries a panel speaker and is held still."
                        ));
                        continue;
                    }
                    let wall_m = field_wall_m.unwrap_or(thickness);
                    let (span_w, span_h) = (width + wall_m, height + wall_m);
                    if span_w <= wall_m || span_h <= wall_m {
                        continue;
                    }
                    thickness_min = thickness_min.min(thickness);
                    thickness_max = thickness_max.max(thickness);
                    let stack = match shared_stack {
                        Some(s) => s,
                        None => {
                            // Per-face stack from the measured thickness; material notes once.
                            let (layers, face_notes) = wall_layers(design, thickness)?;
                            if !measured_notes_done {
                                notes.extend(face_notes);
                                layer_names = layers.iter().map(|l| l.name.clone()).collect();
                                measured_notes_done = true;
                            }
                            let s = WallStack::from_layers(&layers)?;
                            mass_per_area = s.mass_per_area;
                            s
                        }
                    };
                    // Outer skin: march to the outside so an inset air face still places the
                    // source on the cabinet's skin, not inside the wall.
                    let to_outside = {
                        let step = 2.5e-4;
                        let mut t = step;
                        while t <= probe_limit {
                            let p = [
                                centre[0] + t * outward[0],
                                centre[1] + t * outward[1],
                                centre[2] + t * outward[2],
                            ];
                            if !outer.contains_cabinet(p).unwrap_or(true) {
                                break;
                            }
                            t += step;
                        }
                        t
                    };
                    let skin = [
                        centre[0] + to_outside * outward[0],
                        centre[1] + to_outside * outward[1],
                        centre[2] + to_outside * outward[2],
                    ];
                    let mut along = [0.0; 3];
                    along[u_axis] = 1.0;
                    let mut across = [0.0; 3];
                    across[v_axis] = 1.0;
                    let (tiles, used) = wall_tiles(
                        span_w, span_h, &braces, skin, outward, along, across, wall_m,
                    );
                    braces_used += used;
                    for tile in tiles {
                        let panel = WallPanel::new(tile.width, tile.height, &stack)?;
                        let area = panel.element_area_m2();
                        let patches = panel
                            .element_centers_m()
                            .iter()
                            .map(|[x, y]| {
                                let (x, y) = (x + tile.mid_u, y + tile.mid_v);
                                SourcePatch::skin(
                                    [
                                        skin[0] + x * along[0] + y * across[0],
                                        skin[1] + x * along[1] + y * across[1],
                                        skin[2] + x * along[2] + y * across[2],
                                    ],
                                    outward,
                                    area,
                                )
                            })
                            .collect();
                        walls.push(PlacedWall {
                            surface: surface.clone(),
                            chamber: index,
                            panel,
                            patches,
                        });
                    }
                    continue;
                }
                if all_partition {
                    let FaceBeyond::Partition(other, thickness) = probes[0] else {
                        unreachable!()
                    };
                    let same_other = probes
                        .iter()
                        .all(|p| matches!(p, FaceBeyond::Partition(o, _) if *o == other));
                    if !same_other || thickness <= 0.0 {
                        held += 1;
                        continue;
                    }
                    let key = if index < other {
                        (index, other)
                    } else {
                        (other, index)
                    };
                    if !seen_partitions.insert(key) {
                        continue;
                    }
                    let neighbor = &chambers[other];
                    // Overlap of the two chamber faces in the plane of the partition.
                    let u0 = chamber.min_m[u_axis].max(neighbor.min_m[u_axis]);
                    let u1 = chamber.max_m[u_axis].min(neighbor.max_m[u_axis]);
                    let v0 = chamber.min_m[v_axis].max(neighbor.min_m[v_axis]);
                    let v1 = chamber.max_m[v_axis].min(neighbor.max_m[v_axis]);
                    let span_w = u1 - u0;
                    let span_h = v1 - v0;
                    if span_w <= thickness || span_h <= thickness {
                        held += 1;
                        continue;
                    }
                    // The solid between chambers is printed plastic, not the outer wall build.
                    let mut part_notes = Vec::new();
                    let layer =
                        printed_layer(&design.build.material, thickness, infill, &mut part_notes)?;
                    if layer.youngs_pa <= 0.0 {
                        held += 1;
                        continue;
                    }
                    notes.extend(part_notes);
                    let part_stack = WallStack::from_layers(std::slice::from_ref(&layer))?;
                    let panel = WallPanel::new(span_w, span_h, &part_stack)?;
                    partitions.push(Partition {
                        chamber_a: key.0,
                        chamber_b: key.1,
                        panel,
                        thickness_m: thickness,
                    });
                    continue;
                }
                held += 1;
            }
        }
    }
    if walls.is_empty() {
        return Err("no chamber lies against the cabinet's outer wall".into());
    }
    let first = walls
        .iter()
        .map(|w| w.panel.first_resonance_hz())
        .chain(partitions.iter().map(|p| p.panel.first_resonance_hz()))
        .fold(f64::INFINITY, f64::min);
    let resolved = walls
        .iter()
        .map(|w| w.panel.resolved_up_to_hz)
        .chain(partitions.iter().map(|p| p.panel.resolved_up_to_hz))
        .fold(f64::INFINITY, f64::min);
    let names = if layer_names.is_empty() {
        "printed plastic".into()
    } else {
        layer_names.join(" / ")
    };
    notes.insert(
        0,
        format!(
            "Walls vibrate: {} outer walls of {} ({:.1} kg/m2) over {} chambers, each a plate \
             with supported edges over its chamber's side, pushed by that chamber's mean \
             pressure; the lowest wall resonance is {first:.0} Hz, and wall resonances above \
             {resolved:.0} Hz are left out.",
            walls.len(),
            names,
            mass_per_area,
            chambers.len()
        ),
    );
    if field_wall_m.is_none() && thickness_max > 0.0 {
        notes.push(format!(
            "Wall thickness is measured through the imported solid ({:.1} to {:.1} mm).",
            thickness_min * 1000.0,
            thickness_max * 1000.0
        ));
    }
    if !partitions.is_empty() {
        let part_first = partitions
            .iter()
            .map(|p| p.panel.first_resonance_hz())
            .fold(f64::INFINITY, f64::min);
        notes.push(format!(
            "{} partition(s) between chambers vibrate as solid printed plates pushed by the \
             pressure difference (lowest partition resonance {part_first:.0} Hz); they couple \
             the air network and do not radiate outside.",
            partitions.len()
        ));
    }
    if held > 0 {
        notes.push(format!(
            "{held} chamber sides lie against thicker plastic and are held still."
        ));
    }
    if braces_used > 0 {
        notes.push(format!(
            "{braces_used} brace contact(s) split wall plates into smaller supported plates."
        ));
    }
    notes.push(
        "Wall vibration leaves out the air's load on the walls, standing waves' pressure \
         pattern on them, the driver's frame shaking the baffle, and holes in the walls; \
         curved skin does not vibrate in this model."
            .into(),
    );
    Ok(CabinetWalls {
        walls,
        partitions,
        notes,
    })
}
