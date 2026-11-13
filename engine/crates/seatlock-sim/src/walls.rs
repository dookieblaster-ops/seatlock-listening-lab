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

pub struct PlacedWall {
    pub surface: String,
    pub panel: WallPanel,
    /// One patch per element of `panel`, in the panel's element order.
    pub patches: Vec<SourcePatch>,
}

pub struct CabinetWalls {
    pub walls: Vec<PlacedWall>,
    /// Lines for the fidelity label.
    pub notes: Vec<String>,
}

impl CabinetWalls {
    /// Outward volume velocity of all walls together per pascal inside.
    pub fn admittance(&self, freq_hz: f64) -> Complex64 {
        self.walls.iter().map(|w| w.panel.admittance(freq_hz)).sum()
    }

    /// Outward volume velocity of every element per pascal inside, walls in order.
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

    /// The lowest wall resonance, in Hz.
    pub fn first_resonance_hz(&self) -> f64 {
        self.walls
            .iter()
            .map(|w| w.panel.first_resonance_hz())
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

/// The cabinet's flat walls, or the reason they are left rigid.
pub fn cabinet_walls(design: &DesignFile, outer: &dyn OuterShape) -> Result<CabinetWalls, String> {
    let kind = design.speaker.enclosure.outer.kind.as_str();
    if !PRINTED.contains(&kind) {
        return Err(format!(
            "walls of a {kind} cabinet are not modeled yet (only printed cabinets)"
        ));
    }
    let wall_m = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("wall_m")
        .and_then(Value::as_f64)
        .filter(|w| *w > 0.0)
        .ok_or("the cabinet gives no wall thickness")?;
    let (layers, mut notes) = wall_layers(design, wall_m)?;
    let stack = WallStack::from_layers(&layers)?;
    let size = outer.outer_size_m()?;
    let middle = [size[0] / 2.0, size[1] / 2.0, size[2] / 2.0];
    let mut walls = Vec::new();
    let mut approximated = false;
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
        let (span_w, span_h) = (width - wall_m, height - wall_m);
        if span_w <= wall_m || span_h <= wall_m {
            continue;
        }
        let panel = WallPanel::new(span_w, span_h, &stack)?;
        let element_area = panel.element_area_m2();
        let patches = panel
            .element_centers_m()
            .iter()
            .map(|[x, y]| {
                let (x, y) = (x + mid_offset[0], y + mid_offset[1]);
                SourcePatch {
                    position_m: [
                        centre[0] + x * along[0] + y * across[0],
                        centre[1] + x * along[1] + y * across[1],
                        centre[2] + x * along[2] + y * across[2],
                    ],
                    outward,
                    area_m2: element_area,
                }
            })
            .collect();
        walls.push(PlacedWall {
            surface,
            panel,
            patches,
        });
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
    notes.push(
        "Wall vibration leaves out the air's load on the walls, standing waves' pressure \
         pattern on them, the driver's frame shaking the baffle, and the holes and braces in \
         the walls; curved skin does not vibrate in this model."
            .into(),
    );
    if approximated {
        notes
            .push("Walls that are not rectangles are taken as rectangles of the same area.".into());
    }
    Ok(CabinetWalls { walls, notes })
}
