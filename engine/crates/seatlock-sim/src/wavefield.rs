//! 3D wave views for the page (README section 12, Level 3 D): pressure inside the cabinet,
//! around it, and a bass-only room grid, each as a standing wave and (inside / room) a pulse.

use crate::cabinet_grid_of;
use num_complex::Complex64;
use seatlock_bem::{field_pressure, mesh_outer_shape, NeumannSystem};
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::OuterShape;
use seatlock_core::{DesignFile, Registry};
use seatlock_fdtd::fields::{CapturedField, FieldCapture, StandingBand};
use seatlock_fdtd::Grid;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::PI;
use std::time::Instant;

fn clock() -> Option<Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(Instant::now())
    }
}

fn elapsed_ms(started: Option<Instant>) -> f64 {
    started
        .map(|t| t.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[derive(Debug, Clone, Deserialize)]
pub struct WaveFieldRequest {
    /// `inside`, `outside`, or `room`.
    pub domain: String,
    /// Standing-wave frequency in Hz.
    pub frequency_hz: Option<f64>,
    /// Cell size for the inside / outside / room grid.
    pub dx_m: Option<f64>,
    /// Also capture a short pulse animation (inside and room).
    #[serde(default)]
    pub pulse: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaveFieldReport {
    pub domain: String,
    pub frequency_hz: f64,
    pub field: CapturedField,
    pub notes: Vec<String>,
    pub elapsed_ms: f64,
    /// Markers the page draws: packed `[x,y,z,…]`.
    pub markers_m: Vec<f32>,
    pub marker_labels: Vec<String>,
}

/// Runs one wave-field capture for the page.
pub fn wave_field(
    design: &DesignFile,
    request: &WaveFieldRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<WaveFieldReport, String> {
    let started = clock();
    let domain = request.domain.trim().to_ascii_lowercase();
    match domain.as_str() {
        "inside" => inside(design, request, registry, progress, started),
        "outside" => outside(design, request, registry, started),
        "room" => room(design, request, progress, started),
        other => Err(format!(
            "unknown wave-field domain '{other}' (use inside, outside, or room)."
        )),
    }
}

fn inside(
    design: &DesignFile,
    request: &WaveFieldRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
    started: Option<Instant>,
) -> Result<WaveFieldReport, String> {
    let outer = registry.create_outer(
        &design.speaker.enclosure.outer.kind,
        &design.speaker.enclosure.outer.fields,
    )?;
    let dx = request.dx_m.unwrap_or(0.012).clamp(0.008, 0.03);
    let freq = request
        .frequency_hz
        .unwrap_or(default_inside_freq(outer.as_ref()))
        .clamp(40.0, C0 / (8.0 * dx));
    let built = cabinet_grid_of(design, outer.as_ref(), dx)?;
    let mut grid = built.grid;
    grid.true_volume_m3 = built.inside_volume_m3;
    let dt = grid.time_step_s();
    let pulse_every = if request.pulse {
        ((0.0004 / dt).round() as usize).max(1)
    } else {
        0
    };
    let cells = grid.n[0] * grid.n[1] * grid.n[2];
    let options = seatlock_fdtd::grid::Options {
        max_steps: (0.35 / dt).ceil() as usize,
        frequencies_hz: vec![freq],
        progress_every: 200,
        extrapolate: false,
        z_scale: Vec::new(),
        relaxed: Vec::new(),
        loss_per_s: Vec::new(),
        damping: built.damping,
        capture: Some(FieldCapture {
            standing_hz: vec![freq],
            pulse_every,
            pulse_max_frames: 48,
            stride: if cells > 40_000 { 2 } else { 1 },
        }),
    };
    let response = seatlock_fdtd::grid::run(&grid, &options, progress);
    if response.canceled {
        return Err("it was canceled.".into());
    }
    let field = response.field.ok_or("the run captured no field.")?;
    let mut notes = vec![
        format!(
            "Inside air on a {:.0} mm grid ({} kept cells). Standing wave at {:.0} Hz from the 3D air model.",
            dx * 1000.0,
            field.cells.len(),
            freq
        ),
        "The pulse is the same short drive used to find the cabinet's air load; shown in slow motion."
            .into(),
    ];
    if !response.decayed {
        notes.push(
            "The air had not fully quieted when the capture stopped; the pattern is still usable."
                .into(),
        );
    }
    let (markers_m, marker_labels) = driver_marker(design, outer.as_ref());
    Ok(WaveFieldReport {
        domain: "inside".into(),
        frequency_hz: freq,
        field,
        notes,
        elapsed_ms: elapsed_ms(started),
        markers_m,
        marker_labels,
    })
}

fn default_inside_freq(outer: &dyn OuterShape) -> f64 {
    let size = outer.outer_size_m().unwrap_or([0.2, 0.3, 0.25]);
    let longest = size[0].max(size[1]).max(size[2]).max(0.1);
    (C0 / (2.0 * longest)).clamp(120.0, 800.0)
}

fn driver_marker(design: &DesignFile, outer: &dyn OuterShape) -> (Vec<f32>, Vec<String>) {
    let mut markers_m = Vec::new();
    let mut marker_labels = Vec::new();
    if let Some(mount) = design.speaker.enclosure.mounts.first() {
        if let Ok(pose) = outer.mount_pose(&mount.surface, mount.at_m) {
            markers_m.extend([
                pose.position_m[0] as f32,
                pose.position_m[1] as f32,
                pose.position_m[2] as f32,
            ]);
            marker_labels.push("driver".into());
        }
    }
    (markers_m, marker_labels)
}

fn outside(
    design: &DesignFile,
    request: &WaveFieldRequest,
    registry: &Registry,
    started: Option<Instant>,
) -> Result<WaveFieldReport, String> {
    let outer = registry.create_outer(
        &design.speaker.enclosure.outer.kind,
        &design.speaker.enclosure.outer.fields,
    )?;
    let size = outer.outer_size_m()?;
    let freq = request.frequency_hz.unwrap_or(500.0).clamp(80.0, 4000.0);
    let dx = request.dx_m.unwrap_or(0.02).clamp(0.012, 0.04);
    let (mesh, _kind) = mesh_outer_shape(size, 5, |p| outer.contains_cabinet(p))?;

    let mut v_n = vec![Complex64::new(0.0, 0.0); mesh.triangles.len()];
    let mut driven = 0.0_f64;
    if let Some(mount) = design.speaker.enclosure.mounts.first() {
        if let Ok(pose) = outer.mount_pose(&mount.surface, mount.at_m) {
            let radius = 0.05_f64;
            for (tri, area) in mesh.areas.iter().enumerate() {
                let center = mesh.centroids[tri];
                let normal = mesh.normals[tri];
                let d = [
                    center[0] - pose.position_m[0],
                    center[1] - pose.position_m[1],
                    center[2] - pose.position_m[2],
                ];
                let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let face = normal[0] * pose.outward[0]
                    + normal[1] * pose.outward[1]
                    + normal[2] * pose.outward[2];
                if dist <= radius && face > 0.2 {
                    v_n[tri] = Complex64::new(1.0, 0.0);
                    driven += *area;
                }
            }
        }
    }
    if driven < 1e-6 {
        for (tri, normal) in mesh.normals.iter().enumerate() {
            // Front face of the lab's cabinets faces −y (toward the listener).
            if normal[1] < -0.35 {
                v_n[tri] = Complex64::new(1.0, 0.0);
                driven += mesh.areas[tri];
            }
        }
    }
    if driven < 1e-6 {
        return Err("no surface to drive for the outside wave view.".into());
    }
    for v in &mut v_n {
        *v /= driven;
    }
    let system = NeumannSystem::assemble(&mesh, freq)?;
    let p_surf = system.solve_velocity(&v_n)?;
    let omega = 2.0 * PI * freq;
    let q: Vec<_> = v_n
        .iter()
        .map(|&v| Complex64::new(0.0, -omega * RHO0) * v)
        .collect();

    let margin = 0.35_f64;
    let low = [-margin, -margin, -margin];
    let high = [size[0] + margin, size[1] + margin, size[2] + margin];
    let n = [
        ((high[0] - low[0]) / dx).round().max(8.0) as usize,
        ((high[1] - low[1]) / dx).round().max(8.0) as usize,
        ((high[2] - low[2]) / dx).round().max(8.0) as usize,
    ];
    let stride = if n[0] * n[1] * n[2] > 25_000 { 2 } else { 1 };
    let mut cells = Vec::new();
    let mut positions = Vec::new();
    let mut re = Vec::new();
    let mut im = Vec::new();
    let mut max_abs = 0.0_f32;
    let mut index = 0u32;
    for i in (0..n[0]).step_by(stride) {
        for j in (0..n[1]).step_by(stride) {
            for k in (0..n[2]).step_by(stride) {
                let p = [
                    low[0] + (i as f64 + 0.5) * dx,
                    low[1] + (j as f64 + 0.5) * dx,
                    low[2] + (k as f64 + 0.5) * dx,
                ];
                if outer.contains_cabinet(p).unwrap_or(false) {
                    index += 1;
                    continue;
                }
                let clearance = dx * 0.75;
                let near = mesh.centroids.iter().any(|c| {
                    let d0 = c[0] - p[0];
                    let d1 = c[1] - p[1];
                    let d2 = c[2] - p[2];
                    d0 * d0 + d1 * d1 + d2 * d2 < clearance * clearance
                });
                if near {
                    index += 1;
                    continue;
                }
                let press = field_pressure(&mesh, system.k, &p_surf, &q, p)?;
                let r = press.re as f32;
                let i_v = press.im as f32;
                max_abs = max_abs.max((r * r + i_v * i_v).sqrt());
                cells.push(index);
                positions.extend([p[0] as f32, p[1] as f32, p[2] as f32]);
                re.push(r);
                im.push(i_v);
                index += 1;
            }
        }
    }
    let field = CapturedField {
        dx_m: dx,
        origin_m: low,
        n,
        cells,
        positions_m: positions,
        standing: vec![StandingBand {
            frequency_hz: freq,
            re,
            im,
            max_abs,
        }],
        pulse: None,
    };
    let notes = vec![
        format!(
            "Outside air from the boundary-element model at {:.0} Hz ({} probe points, {:.0} mm spacing).",
            freq,
            field.cells.len(),
            dx * 1000.0
        ),
        "Unit volume velocity on the driver face; the rest of the cabinet is rigid. Pulse animation is not shown for the outside (frequency-domain solve)."
            .into(),
    ];
    let (markers_m, marker_labels) = driver_marker(design, outer.as_ref());
    Ok(WaveFieldReport {
        domain: "outside".into(),
        frequency_hz: freq,
        field,
        notes,
        elapsed_ms: elapsed_ms(started),
        markers_m,
        marker_labels,
    })
}

fn room(
    design: &DesignFile,
    request: &WaveFieldRequest,
    progress: &mut dyn FnMut(usize, usize) -> bool,
    started: Option<Instant>,
) -> Result<WaveFieldReport, String> {
    let room = design
        .speaker
        .room
        .as_ref()
        .ok_or("turn on the room estimate first, then show the room wave view.")?;
    if room.kind != "simple" {
        return Err(format!("room kind '{}' has no wave view yet.", room.kind));
    }
    let volume = room
        .fields
        .get("volume_m3")
        .and_then(Value::as_f64)
        .unwrap_or(50.0)
        .max(10.0);
    let wall_behind = room
        .fields
        .get("wall_behind_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.5)
        .clamp(0.2, 3.0);
    // Assumed proportions L:W:H = 1.6 : 1.25 : 1 from the volume (labeled).
    let height = (volume / (1.6 * 1.25)).cbrt();
    let length = 1.6 * height;
    let width = 1.25 * height;
    let freq = request
        .frequency_hz
        .unwrap_or(C0 / (2.0 * length.max(width)))
        .clamp(20.0, 300.0);
    let dx = request.dx_m.unwrap_or(0.2).clamp(0.12, 0.35);
    let n = [
        (length / dx).round().max(6.0) as usize,
        (width / dx).round().max(5.0) as usize,
        (height / dx).round().max(4.0) as usize,
    ];
    let cells_total = n[0] * n[1] * n[2];
    let air = vec![true; cells_total];
    let speaker = [
        wall_behind.clamp(dx, length - dx),
        width * 0.35,
        height * 0.4,
    ];
    let listen_r = design.speaker.listening.distance_m.max(0.5);
    let listener = [
        (speaker[0] + listen_r).min(length - dx),
        width * 0.55,
        height * 0.45,
    ];
    let src_i = ((speaker[0] / dx).floor() as usize).min(n[0].saturating_sub(1));
    let src_j = ((speaker[1] / dx).floor() as usize).min(n[1].saturating_sub(1));
    let src_k = ((speaker[2] / dx).floor() as usize).min(n[2].saturating_sub(1));
    let source_cell = (src_i * n[1] + src_j) * n[2] + src_k;
    let grid = Grid {
        n,
        dx,
        origin_m: [0.0, 0.0, 0.0],
        air,
        source: vec![source_cell],
        absorbing: Vec::new(),
        openings: Vec::new(),
        true_volume_m3: volume,
    };
    let dt = grid.time_step_s();
    let pulse_every = if request.pulse {
        ((0.0012 / dt).round() as usize).max(1)
    } else {
        0
    };
    let options = seatlock_fdtd::grid::Options {
        max_steps: (0.6 / dt).ceil() as usize,
        frequencies_hz: vec![freq],
        progress_every: 400,
        extrapolate: false,
        z_scale: Vec::new(),
        relaxed: Vec::new(),
        loss_per_s: vec![8.0],
        damping: Vec::new(),
        capture: Some(FieldCapture {
            standing_hz: vec![freq],
            pulse_every,
            pulse_max_frames: 40,
            stride: 1,
        }),
    };
    let response = seatlock_fdtd::grid::run(&grid, &options, progress);
    if response.canceled {
        return Err("it was canceled.".into());
    }
    let field = response.field.ok_or("the run captured no field.")?;
    let notes = vec![
        format!(
            "Bass-only room grid {:.2} × {:.2} × {:.2} m ({:.0} m³), {:.0} cm cells, standing wave at {:.0} Hz.",
            length, width, height, volume, dx * 100.0, freq
        ),
        "Room proportions are assumed from the volume (1.6 : 1.25 : 1). The everyday room estimate on the graphs is unchanged."
            .into(),
        "Rigid walls with a light air loss so the pattern settles; below about 300 Hz only."
            .into(),
    ];
    Ok(WaveFieldReport {
        domain: "room".into(),
        frequency_hz: freq,
        field,
        notes,
        elapsed_ms: elapsed_ms(started),
        markers_m: vec![
            speaker[0] as f32,
            speaker[1] as f32,
            speaker[2] as f32,
            listener[0] as f32,
            listener[1] as f32,
            listener[2] as f32,
        ],
        marker_labels: vec!["speaker".into(), "listener".into()],
    })
}
