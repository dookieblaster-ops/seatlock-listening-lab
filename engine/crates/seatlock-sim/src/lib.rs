//! Turns a design into simulation results, score, cost, and optimizer output.

use std::time::Instant;

use num_complex::Complex64;
use seatlock_acoustics::{
    cone_displacement_m3, effective_port_length_m, spl_db, vented_sample, PortSpec,
};
use seatlock_core::catalog;
use seatlock_core::constants::C0;
use seatlock_core::design::{DesignFile, KindPart};
use seatlock_core::grid::default_grid;
use seatlock_core::issue::Issue;
use seatlock_core::registry::{DspBlock, OuterShape, Registry};
use seatlock_correct::metrics;
use seatlock_dsp::{delay_block, gain_block, polarity_block, prefer_inverted_high, ThiranDelay};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub fn builtin() -> Registry {
    let mut registry = Registry::new();
    seatlock_geom::register(&mut registry);
    seatlock_acoustics::register(&mut registry);
    seatlock_dsp::register(&mut registry);
    registry
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulateRequest {
    #[serde(default = "default_enclosure")]
    pub enclosure: String,
    #[serde(default = "default_radiation")]
    pub radiation: String,
    #[serde(default = "default_space")]
    pub space: String,
    #[serde(default = "default_voltage")]
    pub voltage_v: f64,
    #[serde(default)]
    pub frequencies_hz: Option<Vec<f64>>,
    #[serde(default)]
    pub revision: u64,
}

fn default_enclosure() -> String {
    "lumped".to_string()
}
fn default_radiation() -> String {
    "point".to_string()
}
fn default_space() -> String {
    "half".to_string()
}
fn default_voltage() -> f64 {
    2.83
}

impl Default for SimulateRequest {
    fn default() -> Self {
        Self {
            enclosure: default_enclosure(),
            radiation: default_radiation(),
            space: default_space(),
            voltage_v: default_voltage(),
            frequencies_hz: None,
            revision: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplexCurve {
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fidelity {
    pub enclosure: String,
    pub radiation: String,
    pub room: String,
    pub ignored: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RadiatorCurve {
    pub index: usize,
    pub kind: String,
    pub spl_db: Vec<f64>,
    pub impedance_ohm: Vec<f64>,
    pub cone_travel_m: Vec<f64>,
    pub port_speed_m: Vec<f64>,
    pub max_spl_db: Vec<f64>,
    pub pressure: ComplexCurve,
    pub fidelity: Fidelity,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulateResult {
    pub revision: u64,
    pub frequencies_hz: Vec<f64>,
    pub interior_volume_m3: Option<f64>,
    pub summed_spl_db: Vec<f64>,
    pub summed_pressure: ComplexCurve,
    pub radiators: Vec<RadiatorCurve>,
    pub issues: Vec<Issue>,
    pub fidelity: Fidelity,
    pub cost_note: String,
    pub elapsed_ms: f64,
    pub f6_hz: f64,
    pub flatness_db: f64,
    pub max_spl_metric_db: f64,
}

pub fn simulate(
    design: &DesignFile,
    request: &SimulateRequest,
    registry: &Registry,
) -> SimulateResult {
    let started = simulation_clock();
    let freqs = request.frequencies_hz.clone().unwrap_or_else(default_grid);
    let mut issues = Vec::new();
    let check = seatlock_core::design_check(
        &seatlock_core::design::canonical_json(design).unwrap_or_default(),
        registry,
    );
    if let Ok(checked) = check {
        issues.extend(checked.issues);
    }

    let mut ignored = vec![
        "no cone break-up".to_string(),
        "the sealed-box model ignores cabinet shape and standing waves".to_string(),
    ];
    if request.radiation == "point" {
        ignored.push("source direction is ignored".to_string());
    }
    if request.space == "half" || request.space == "full" || request.space == "step" {
    } else {
        issues.push(Issue::warning(
            "unsupported_request",
            format!(
                "Unknown radiation space \"{}\". Using half space.",
                request.space
            ),
            "request.space",
        ));
    }
    if !design.speaker.enclosure.ducts.is_empty() {
        ignored.push("ducts were left out of the sealed box".to_string());
        issues.push(Issue::warning(
            "unsupported_kind",
            "Openings are not part of the sealed-box model yet, so they were left out.",
            "speaker.enclosure.ducts",
        ));
    }
    let faces_listener = design
        .speaker
        .enclosure
        .mounts
        .iter()
        .all(|mount| mount.surface == "front");
    if !faces_listener {
        issues.push(Issue::warning(
            "unsupported_kind",
            "A sound source does not face the listener. This quick model treats it as if it did.",
            "speaker.enclosure.mounts",
        ));
    }

    let mut fidelity = Fidelity {
        enclosure: request.enclosure.clone(),
        radiation: request.radiation.clone(),
        room: design
            .speaker
            .room
            .as_ref()
            .map(|room| room.kind.clone())
            .unwrap_or_else(|| "free".to_string()),
        ignored: ignored.clone(),
    };

    let fail = |issues: Vec<Issue>, fidelity: Fidelity| SimulateResult {
        revision: request.revision,
        frequencies_hz: freqs.clone(),
        interior_volume_m3: None,
        summed_spl_db: Vec::new(),
        summed_pressure: ComplexCurve {
            re: Vec::new(),
            im: Vec::new(),
        },
        radiators: Vec::new(),
        issues,
        fidelity,
        cost_note: catalog::cost_note(design),
        elapsed_ms: elapsed_ms(started),
        f6_hz: 0.0,
        flatness_db: 0.0,
        max_spl_metric_db: 0.0,
    };

    let outer = match registry.create_outer(
        &design.speaker.enclosure.outer.kind,
        &design.speaker.enclosure.outer.fields,
    ) {
        Ok(outer) => outer,
        Err(err) => {
            issues.push(Issue::error(
                "unsupported_kind",
                err,
                "speaker.enclosure.outer",
            ));
            return fail(issues, fidelity);
        }
    };
    let gross = match outer.interior_volume_m3() {
        Ok(volume) => volume,
        Err(err) => {
            issues.push(Issue::error(
                "missing_field",
                err,
                "speaker.enclosure.outer",
            ));
            return fail(issues, fidelity);
        }
    };
    let mut displacement = 0.0;
    for radiator in &design.speaker.radiators {
        if radiator.kind == "cone" {
            let (amount, _, _) = cone_displacement_m3(&radiator.fields);
            displacement += amount;
        }
    }
    let volume = (gross - displacement).max(0.0);
    let baffle = outer.baffle_width_m();

    let enclosure = match registry.create_enclosure(&request.enclosure, &Map::new()) {
        Ok(model) => model,
        Err(err) => {
            issues.push(Issue::error(
                "unsupported_request",
                err,
                "request.enclosure",
            ));
            return fail(issues, fidelity);
        }
    };
    let ports = design_ports(design);
    let (z_ab, vented) = if ports.is_empty() {
        (enclosure.z_ab(volume, &freqs), None)
    } else {
        let mut impedance = Vec::with_capacity(freqs.len());
        let mut samples = Vec::with_capacity(freqs.len());
        for freq in &freqs {
            let sample = vented_sample(volume, &ports, *freq, false);
            impedance.push(sample.z_ab);
            samples.push(sample);
        }
        fidelity.ignored.push(
            "The quick model ignores resonances inside the port, and treats a bend as a straight tube."
                .into(),
        );
        (impedance, Some(samples))
    };
    let silent = vec![Complex64::new(0.0, 0.0); freqs.len()];
    let radiation = match registry.create_radiation(&request.radiation, &Map::new()) {
        Ok(model) => model,
        Err(err) => {
            issues.push(Issue::error(
                "unsupported_request",
                err,
                "request.radiation",
            ));
            return fail(issues, fidelity);
        }
    };
    let room_kind = design
        .speaker
        .room
        .as_ref()
        .map(|room| room.kind.as_str())
        .unwrap_or("free");
    let room_fields = design
        .speaker
        .room
        .as_ref()
        .map(|room| room.fields.clone())
        .unwrap_or_default();
    let room = match registry.create_room(room_kind, &room_fields) {
        Ok(model) => model,
        Err(err) => {
            issues.push(Issue::warning("unsupported_kind", err, "speaker.room"));
            match registry.create_room("free", &Map::new()) {
                Ok(model) => model,
                Err(err) => {
                    issues.push(Issue::error("unsupported_kind", err, "speaker.room"));
                    return fail(issues, fidelity);
                }
            }
        }
    };

    let sample_rate = design.dsp.sample_rate_hz as f64;
    let whole = chain_response(
        &design.dsp.whole_speaker,
        sample_rate,
        registry,
        "dsp.whole_speaker",
        &mut issues,
        &freqs,
    );
    let amp_limit = amplifier_voltage_limit(design);

    let mut radiators = Vec::new();
    for (index, part) in design.speaker.radiators.iter().enumerate() {
        let radiator = match registry.create_radiator(&part.kind, &part.fields) {
            Ok(radiator) => radiator,
            Err(_) => continue,
        };
        let loads_box = driver_loads_box(part);
        let eval = radiator.evaluate(&freqs, if loads_box { &z_ab } else { &silent });
        issues.extend(eval.warnings);
        let chain = design.dsp.per_radiator.get(index);
        let mut blocks_h = whole.clone();
        if let Some(chain) = chain {
            let extra = chain_response(
                &chain.blocks,
                sample_rate,
                registry,
                &format!("dsp.per_radiator[{index}].blocks"),
                &mut issues,
                &freqs,
            );
            multiply_into(&mut blocks_h, &extra);
            let samples = chain.delay_s * sample_rate;
            let whole = samples.floor().max(0.0);
            let fraction = samples - whole;
            let mut tail: Vec<Box<dyn DspBlock>> = vec![
                delay_block(whole / sample_rate, sample_rate),
                gain_block(chain.gain_db),
                polarity_block(chain.invert),
            ];
            if fraction > 1e-4 {
                tail.push(Box::new(ThiranDelay::new(fraction)));
            }
            if let Some(volts) = amp_limit {
                let mut fields = Map::new();
                fields.insert("threshold_v".into(), serde_json::json!(volts));
                if let Ok(block) = registry.create_dsp("limiter", &fields) {
                    tail.push(block);
                }
            }
            let tail_h = product(&mut tail, &freqs, sample_rate);
            multiply_into(&mut blocks_h, &tail_h);
        }
        let mut sources = vec![eval.volume_velocity.clone()];
        let mut port_speed_1v: Vec<f64> = vec![0.0; freqs.len()];
        if loads_box {
            if let Some(samples) = &vented {
                for (port_index, port) in ports.iter().enumerate() {
                    let mut velocity = Vec::with_capacity(freqs.len());
                    for (bin, sample) in samples.iter().enumerate() {
                        let transfer = sample
                            .h_each
                            .get(port_index)
                            .copied()
                            .unwrap_or(Complex64::new(0.0, 0.0));
                        let outward = -transfer * eval.volume_velocity[bin];
                        let peak = std::f64::consts::SQRT_2 * outward.norm() / port.area_m2;
                        port_speed_1v[bin] = port_speed_1v[bin].max(peak);
                        velocity.push(outward);
                    }
                    sources.push(velocity);
                }
            }
        }
        let pressure_1v = radiation.pressure(
            &sources,
            &freqs,
            design.speaker.listening.distance_m,
            &request.space,
            baffle,
        );
        let mut pressure = Vec::with_capacity(freqs.len());
        let mut spl = Vec::with_capacity(freqs.len());
        let mut travel = Vec::with_capacity(freqs.len());
        let mut port_speed = Vec::with_capacity(freqs.len());
        let mut max_spl = Vec::with_capacity(freqs.len());
        let mut impedance = Vec::with_capacity(freqs.len());
        for i in 0..freqs.len() {
            let drive = Complex64::new(request.voltage_v, 0.0) * blocks_h[i];
            let p = pressure_1v[i] * drive;
            pressure.push(p);
            spl.push(spl_db(p));
            travel.push(eval.excursion_m[i] * drive.norm());
            port_speed.push(port_speed_1v[i] * drive.norm());
            impedance.push(eval.impedance[i].norm());
            let mut clean = max_spl_db(SplLimit {
                spl_1v: spl_db(pressure_1v[i]),
                excursion_1v: eval.excursion_m[i],
                current_1v: eval.current_a[i].norm(),
                xmax_m: eval.xmax_m,
                re_ohm: eval.re_ohm,
                pe_w: eval.pe_w,
                has_excursion_limit: eval.has_excursion_limit,
                amp_limit,
            });
            if port_speed_1v[i] > 1e-9 {
                let volts = (0.05 * C0) / port_speed_1v[i];
                clean = clean.min(spl_db(pressure_1v[i]) + 20.0 * volts.max(1e-12).log10());
            }
            max_spl.push(clean);
        }
        let listened = room.at_listener(&pressure);
        let listened_spl: Vec<f64> = listened.iter().copied().map(spl_db).collect();
        radiators.push(RadiatorCurve {
            index,
            kind: part.kind.clone(),
            spl_db: listened_spl,
            impedance_ohm: impedance,
            cone_travel_m: travel,
            port_speed_m: port_speed,
            max_spl_db: max_spl,
            pressure: curve_from(&listened),
            fidelity: fidelity.clone(),
        });
    }

    if let Some((index, crossover_hz)) = highest_crossover(design) {
        if let Some(branch) = radiators.get(index) {
            let mut others = vec![Complex64::new(0.0, 0.0); freqs.len()];
            for (other_index, other) in radiators.iter().enumerate() {
                if other_index == index {
                    continue;
                }
                for (slot, (re, im)) in others
                    .iter_mut()
                    .zip(other.pressure.re.iter().zip(&other.pressure.im))
                {
                    *slot += Complex64::new(*re, *im);
                }
            }
            let branch_h: Vec<Complex64> = branch
                .pressure
                .re
                .iter()
                .zip(&branch.pressure.im)
                .map(|(re, im)| Complex64::new(*re, *im))
                .collect();
            if prefer_inverted_high(&others, &branch_h, &freqs, crossover_hz) {
                if let Some(branch) = radiators.get_mut(index) {
                    for value in branch
                        .pressure
                        .re
                        .iter_mut()
                        .chain(branch.pressure.im.iter_mut())
                    {
                        *value = -*value;
                    }
                    branch.spl_db = branch
                        .pressure
                        .re
                        .iter()
                        .zip(&branch.pressure.im)
                        .map(|(re, im)| spl_db(Complex64::new(*re, *im)))
                        .collect();
                }
                issues.push(Issue::warning(
                    "polarity",
                    format!(
                        "The driver on branch {index} was flipped so the crossover near {crossover_hz:.0} Hz adds instead of cancelling."
                    ),
                    format!("dsp.per_radiator[{index}].invert"),
                ));
            }
        }
    }

    // Sum the per-radiator listener pressures so a room model stays linear.
    let mut summed_pressure = vec![Complex64::new(0.0, 0.0); freqs.len()];
    for radiator in &radiators {
        for (slot, (re, im)) in summed_pressure
            .iter_mut()
            .zip(radiator.pressure.re.iter().zip(&radiator.pressure.im))
        {
            *slot += Complex64::new(*re, *im);
        }
    }
    let summed_spl_db: Vec<f64> = summed_pressure.iter().copied().map(spl_db).collect();
    let mut max_spl_sum = vec![f64::MAX; freqs.len()];
    for radiator in &radiators {
        for (slot, level) in max_spl_sum.iter_mut().zip(&radiator.max_spl_db) {
            *slot = slot.min(*level);
        }
    }
    let (f6_hz, flatness_db, max_spl_metric_db) = metrics(&freqs, &summed_spl_db, &max_spl_sum);

    SimulateResult {
        revision: request.revision,
        frequencies_hz: freqs,
        interior_volume_m3: Some(volume),
        summed_spl_db,
        summed_pressure: curve_from(&summed_pressure),
        radiators,
        issues,
        fidelity,
        cost_note: catalog::cost_note(design),
        elapsed_ms: elapsed_ms(started),
        f6_hz,
        flatness_db,
        max_spl_metric_db,
    }
}

fn simulation_clock() -> Option<Instant> {
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
        .map(|instant| instant.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn curve_from(values: &[Complex64]) -> ComplexCurve {
    ComplexCurve {
        re: values.iter().map(|value| value.re).collect(),
        im: values.iter().map(|value| value.im).collect(),
    }
}

fn amplifier_voltage_limit(design: &DesignFile) -> Option<f64> {
    let id = design.electronics.amplifiers.first()?;
    let amp = catalog::amplifier(id)?;
    let power = amp.get("power_w")?.as_f64()?;
    let load = amp.get("rated_load_ohm")?.as_f64()?;
    Some((power * load).sqrt())
}

struct SplLimit {
    spl_1v: f64,
    excursion_1v: f64,
    current_1v: f64,
    xmax_m: f64,
    re_ohm: f64,
    pe_w: f64,
    has_excursion_limit: bool,
    amp_limit: Option<f64>,
}

fn max_spl_db(input: SplLimit) -> f64 {
    let mut limit = f64::MAX;
    if input.has_excursion_limit && input.excursion_1v > 1e-15 {
        limit = limit.min(input.xmax_m / input.excursion_1v);
    }
    if input.re_ohm > 0.0 && input.current_1v > 1e-15 {
        let current_max = (input.pe_w / input.re_ohm).sqrt();
        limit = limit.min(current_max / input.current_1v);
    }
    let amp_limit = input.amp_limit;
    let spl_1v = input.spl_1v;
    if let Some(volts) = amp_limit {
        limit = limit.min(volts);
    }
    if !limit.is_finite() || limit <= 0.0 {
        return spl_1v;
    }
    spl_1v + 20.0 * limit.log10()
}

fn highest_crossover(design: &DesignFile) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (index, chain) in design.dsp.per_radiator.iter().enumerate() {
        for block in &chain.blocks {
            if block.kind != "crossover" {
                continue;
            }
            if block.fields.get("band").and_then(Value::as_str) != Some("high") {
                continue;
            }
            let freq = block.fields.get("f_hz").and_then(Value::as_f64)?;
            if best.is_none_or(|(_, current)| freq >= current) {
                best = Some((index, freq));
            }
        }
    }
    best
}

fn driver_loads_box(part: &seatlock_core::design::KindPart) -> bool {
    let Some(id) = part.fields.get("driver_id").and_then(Value::as_str) else {
        return true;
    };
    let sealed = catalog::driver(id)
        .and_then(|record| record.get("sealed_back").and_then(Value::as_bool))
        .unwrap_or(false);
    !sealed
}

fn design_ports(design: &DesignFile) -> Vec<PortSpec> {
    let mut ports = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        let from = duct.get("from").and_then(Value::as_str).unwrap_or("");
        let to = duct.get("to").and_then(Value::as_str).unwrap_or("");
        let leaves = (from == "main" && to == "outside") || (from == "outside" && to == "main");
        if !leaves {
            continue;
        }
        let diameter = duct
            .pointer("/section/start_m/0")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if diameter <= 0.0 {
            continue;
        }
        let mut length = 0.0;
        if let Some(points) = duct.get("path_m").and_then(Value::as_array) {
            for pair in points.windows(2) {
                let a = point3(&pair[0]);
                let b = point3(&pair[1]);
                let dx = a.0 - b.0;
                let dy = a.1 - b.1;
                let dz = a.2 - b.2;
                length += (dx * dx + dy * dy + dz * dz).sqrt();
            }
        }
        let radius = diameter / 2.0;
        ports.push(PortSpec {
            area_m2: std::f64::consts::PI * radius * radius,
            length_eff_m: effective_port_length_m(length, radius),
        });
    }
    ports
}

fn point3(value: &Value) -> (f64, f64, f64) {
    let coord = |index: usize| {
        value
            .as_array()
            .and_then(|point| point.get(index))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    (coord(0), coord(1), coord(2))
}

fn chain_response(
    parts: &[seatlock_core::design::KindPart],
    sample_rate: f64,
    registry: &Registry,
    path: &str,
    issues: &mut Vec<Issue>,
    freqs: &[f64],
) -> Vec<Complex64> {
    let mut blocks = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        match registry.create_dsp(&part.kind, &part.fields) {
            Ok(block) => blocks.push(block),
            Err(err) => {
                if registry.knows(seatlock_core::registry::Family::DspBlock, &part.kind) {
                    issues.push(Issue::error(
                        "missing_field",
                        err,
                        format!("{path}[{index}]"),
                    ));
                }
            }
        }
    }
    product(&mut blocks, freqs, sample_rate)
}

fn product(blocks: &mut [Box<dyn DspBlock>], freqs: &[f64], sample_rate: f64) -> Vec<Complex64> {
    let mut total = vec![Complex64::new(1.0, 0.0); freqs.len()];
    for block in blocks {
        multiply_into(&mut total, &block.frequency_response(freqs, sample_rate));
    }
    total
}

fn multiply_into(total: &mut [Complex64], factor: &[Complex64]) {
    for (slot, value) in total.iter_mut().zip(factor) {
        *slot *= *value;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MountView {
    pub surface: String,
    pub at_m: [f64; 2],
    pub position_m: [f64; 3],
    pub outward: [f64; 3],
    pub cutout_diameter_m: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeometryReport {
    pub width_m: f64,
    pub height_m: f64,
    pub depth_m: f64,
    pub wall_m: f64,
    pub interior_volume_m3: f64,
    pub solid_volume_m3: f64,
    pub voxel_air_volume_m3: f64,
    pub print_mass_kg: f64,
    pub cost_note: String,
    pub mounts: Vec<MountView>,
}

pub fn geometry_report(design: &DesignFile) -> Result<GeometryReport, String> {
    let outer = &design.speaker.enclosure.outer;
    if outer.kind != "box" {
        return Err(format!("geometry report needs a box, found {}", outer.kind));
    }
    let shape = seatlock_geom::BoxShape {
        width_m: outer
            .fields
            .get("width_m")
            .and_then(|v| v.as_f64())
            .ok_or("missing width_m")?,
        height_m: outer
            .fields
            .get("height_m")
            .and_then(|v| v.as_f64())
            .ok_or("missing height_m")?,
        depth_m: outer
            .fields
            .get("depth_m")
            .and_then(|v| v.as_f64())
            .ok_or("missing depth_m")?,
        wall_m: outer
            .fields
            .get("wall_m")
            .and_then(|v| v.as_f64())
            .ok_or("missing wall_m")?,
    };
    let mut cutout_area = 0.0;
    let mut mounts = Vec::new();
    for mount in &design.speaker.enclosure.mounts {
        let pose = shape.mount_pose(&mount.surface, mount.at_m)?;
        let cutout = design
            .speaker
            .radiators
            .iter()
            .find(|radiator| {
                radiator.fields.get("mount").and_then(|v| v.as_str()) == Some(mount.id.as_str())
            })
            .and_then(|radiator| radiator.fields.get("driver_id").and_then(|v| v.as_str()))
            .and_then(catalog::driver)
            .and_then(|driver| {
                driver
                    .pointer("/mounting/cutout_diameter_m")
                    .and_then(|v| v.as_f64())
            })
            .unwrap_or(0.0);
        cutout_area += std::f64::consts::PI * (cutout * 0.5).powi(2);
        mounts.push(MountView {
            surface: mount.surface.clone(),
            at_m: mount.at_m,
            position_m: pose.position_m,
            outward: pose.outward,
            cutout_diameter_m: cutout,
        });
    }
    let solid = shape.solid_volume_m3(cutout_area)?;
    let density = catalog::material(&design.build.material)
        .and_then(|record| record.get("density_kg_m3").and_then(|v| v.as_f64()))
        .unwrap_or(0.0);
    let print_mass_kg = solid * density * design.build.infill_fraction;
    Ok(GeometryReport {
        width_m: shape.width_m,
        height_m: shape.height_m,
        depth_m: shape.depth_m,
        wall_m: shape.wall_m,
        interior_volume_m3: shape.interior_volume_m3()?,
        solid_volume_m3: solid,
        voxel_air_volume_m3: shape.voxel_air_volume_m3(0.005)?,
        print_mass_kg,
        cost_note: catalog::cost_note(design),
        mounts,
    })
}

fn box_alignment(design: &DesignFile, volume_m3: f64) -> (f64, f64) {
    let Some(id) = design
        .speaker
        .radiators
        .first()
        .and_then(|radiator| radiator.fields.get("driver_id"))
        .and_then(|value| value.as_str())
    else {
        return (50.0, 0.7);
    };
    let Some(driver) = catalog::driver(id) else {
        return (50.0, 0.7);
    };
    let published = driver.get("published");
    let number = |key: &str| {
        published
            .and_then(|item| item.get(key))
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0)
    };
    let fs = number("fs_hz");
    let qes = number("qes");
    let qms = number("qms");
    let vas = number("vas_m3");
    let qts = qes * qms / (qes + qms).max(1e-9);
    let ratio = (1.0 + vas / volume_m3.max(1e-6)).sqrt();
    (fs * ratio, qts * ratio)
}

pub fn correction_plan(design: &DesignFile, registry: &Registry) -> Value {
    let level = catalog::target(&design.target)
        .and_then(|target| {
            target
                .get("max_spl_db_min")
                .and_then(|value| value.as_f64())
        })
        .unwrap_or(95.0);
    let freqs = seatlock_core::grid::log_grid(10.0, 20_000.0, 24.0);
    let result = simulate(
        design,
        &SimulateRequest {
            frequencies_hz: Some(freqs.clone()),
            ..SimulateRequest::default()
        },
        registry,
    );
    if result.radiators.is_empty() {
        return serde_json::json!({ "filters": [], "message": "There is no driver to correct." });
    }
    let alignment = box_alignment(design, result.interior_volume_m3.unwrap_or(0.01));
    let ports = design_ports(design);
    let tuning = if ports.is_empty() {
        0.0
    } else {
        seatlock_acoustics::tuning_hz(result.interior_volume_m3.unwrap_or(0.01), &ports)
    };
    let protected = design.dsp.correction.protected_band_hz;
    let mut plans = Vec::new();
    for (index, curve) in result.radiators.iter().enumerate() {
        let fixed = fixed_block_shape(design, registry, &freqs, index);
        let outcome = seatlock_correct::correct(
            &freqs,
            &curve.spl_db,
            &curve.max_spl_db,
            level,
            protected,
            design.dsp.correction.max_filters as usize,
            design.dsp.sample_rate_hz as f64,
            alignment.0,
            alignment.1,
            !ports.is_empty(),
            tuning,
            &fixed,
        );
        plans.push(serde_json::json!({
            "message": outcome.message,
            "filters": outcome.filters.iter().map(filter_json).collect::<Vec<_>>(),
        }));
    }
    let first = plans.first().cloned().unwrap_or(serde_json::json!({}));
    serde_json::json!({
        "message": first["message"],
        "filters": first["filters"],
        "per_radiator": plans,
    })
}

fn filter_json(filter: &seatlock_correct::CorrectionFilter) -> Value {
    serde_json::json!({
        "kind": "biquad",
        "type": filter.filter_type,
        "f_hz": filter.f_hz,
        "q": filter.q,
        "gain_db": filter.gain_db,
        "from_hz": filter.from_hz,
        "from_q": filter.from_q,
        "reason": filter.reason,
        "correction": true
    })
}

fn fixed_block_shape(
    design: &DesignFile,
    registry: &Registry,
    freqs: &[f64],
    radiator: usize,
) -> Vec<f64> {
    let mut parts = design.dsp.whole_speaker.clone();
    if let Some(chain) = design.dsp.per_radiator.get(radiator) {
        for block in &chain.blocks {
            let correction = block
                .fields
                .get("correction")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !correction {
                parts.push(block.clone());
            }
        }
    }
    if parts.is_empty() {
        return vec![0.0; freqs.len()];
    }
    let response = chain_response(
        &parts,
        design.dsp.sample_rate_hz as f64,
        registry,
        "dsp",
        &mut Vec::new(),
        freqs,
    );
    let mut db: Vec<f64> = response
        .iter()
        .map(|value| 20.0 * value.norm().max(1e-12).log10())
        .collect();
    let mut sum = 0.0;
    let mut count = 0.0;
    for (freq, level) in freqs.iter().zip(&db) {
        if *freq >= 300.0 && *freq <= 3_000.0 {
            sum += *level;
            count += 1.0;
        }
    }
    if count > 0.0 {
        let mean = sum / count;
        for level in &mut db {
            *level -= mean;
        }
    }
    db
}

/// Coefficients, plain-language filter text, and optional impulse responses for listen mode.
///
/// `impulse_len` is the FFT length. Pass 0 to skip the impulse (filter export only).
pub fn listen_package(
    design: &DesignFile,
    registry: &Registry,
    sample_rate_hz: f64,
    correction_on: bool,
    impulse_len: usize,
) -> Value {
    let sample_rate_hz = if sample_rate_hz > 1.0 {
        sample_rate_hz
    } else {
        design.dsp.sample_rate_hz as f64
    };
    let mut channels = Vec::new();
    let mut lines = Vec::new();
    let radiators = design.speaker.radiators.len().max(1);
    for index in 0..radiators {
        let mut stages = Vec::new();
        let mut gain = 1.0;
        let mut delay = 0usize;
        lines.push(format!("Radiator {}", index + 1));
        append_parts(
            &design.dsp.whole_speaker,
            sample_rate_hz,
            correction_on,
            &mut stages,
            &mut gain,
            &mut delay,
            &mut lines,
        );
        if let Some(chain) = design.dsp.per_radiator.get(index) {
            append_parts(
                &chain.blocks,
                sample_rate_hz,
                correction_on,
                &mut stages,
                &mut gain,
                &mut delay,
                &mut lines,
            );
            if chain.delay_s.abs() > 1e-9 {
                let samples = (chain.delay_s * sample_rate_hz).round().max(0.0) as usize;
                delay += samples;
                lines.push(format!("Delay of {samples} samples."));
            }
            if chain.gain_db.abs() > 1e-9 {
                gain *= 10_f64.powf(chain.gain_db / 20.0);
                lines.push(format!("Level change of {:.1} dB.", chain.gain_db));
            }
            if chain.invert {
                gain = -gain;
                lines.push("Polarity inverted.".to_string());
            }
        }
        if stages.is_empty() && (gain - 1.0).abs() < 1e-9 && delay == 0 {
            lines.push("No filters on this radiator. The signal reaches it unchanged.".to_string());
        }
        channels.push(serde_json::json!({
            "coeffs": stages,
            "gain": gain,
            "delay_samples": delay,
        }));
    }
    let impulses = if impulse_len >= 32 {
        impulse_responses(design, registry, sample_rate_hz, impulse_len)
    } else {
        Vec::new()
    };
    let level = simulate(
        design,
        &SimulateRequest {
            voltage_v: 2.83,
            ..SimulateRequest::default()
        },
        registry,
    );
    let spl_1k = level
        .radiators
        .first()
        .and_then(|curve| {
            curve
                .spl_db
                .iter()
                .zip(&level.frequencies_hz)
                .min_by(|(_, a), (_, b)| {
                    (*a - 1000.0)
                        .abs()
                        .partial_cmp(&(*b - 1000.0).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(spl, _)| *spl)
        })
        .unwrap_or(0.0);
    serde_json::json!({
        "sample_rate_hz": sample_rate_hz,
        "channels": channels,
        "impulses": impulses,
        "filter_text": lines.join("\n"),
        "spl_1k_db": spl_1k,
        "correction_on": correction_on,
    })
}

fn append_parts(
    parts: &[KindPart],
    sample_rate_hz: f64,
    correction_on: bool,
    stages: &mut Vec<[f64; 5]>,
    gain: &mut f64,
    delay: &mut usize,
    lines: &mut Vec<String>,
) {
    for part in parts {
        let is_correction = part
            .fields
            .get("correction")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if is_correction && !correction_on {
            continue;
        }
        match part.kind.as_str() {
            "biquad" => {
                let kind = part
                    .fields
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("peaking");
                let freq = part
                    .fields
                    .get("f_hz")
                    .and_then(Value::as_f64)
                    .unwrap_or(1_000.0);
                let q = part
                    .fields
                    .get("q")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.707);
                let gain_db = part
                    .fields
                    .get("gain_db")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                let from_hz = part
                    .fields
                    .get("from_hz")
                    .and_then(Value::as_f64)
                    .unwrap_or(freq);
                let from_q = part
                    .fields
                    .get("from_q")
                    .and_then(Value::as_f64)
                    .unwrap_or(q);
                match seatlock_dsp::biquad_coefficients(
                    kind,
                    freq,
                    q,
                    gain_db,
                    from_hz,
                    from_q,
                    sample_rate_hz,
                ) {
                    Ok((b0, b1, b2, a1, a2)) => {
                        stages.push([b0, b1, b2, a1, a2]);
                        let reason = part
                            .fields
                            .get("reason")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if reason.is_empty() {
                            lines.push(format!(
                                "biquad {kind} at {freq:.1} Hz, Q {q:.3}, gain {gain_db:.1} dB."
                            ));
                        } else {
                            lines.push(format!(
                                "biquad {kind} at {freq:.1} Hz, Q {q:.3}, gain {gain_db:.1} dB. {reason}"
                            ));
                        }
                        lines.push(format!("{b0:.8} {b1:.8} {b2:.8} {a1:.8} {a2:.8}"));
                    }
                    Err(err) => lines.push(format!("A {kind} filter was left out: {err}")),
                }
            }
            "gain" => {
                let gain_db = part
                    .fields
                    .get("gain_db")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                *gain *= 10_f64.powf(gain_db / 20.0);
                lines.push(format!("Gain of {gain_db:.1} dB."));
            }
            "polarity" => {
                let invert = part
                    .fields
                    .get("invert")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                if invert {
                    *gain = -*gain;
                    lines.push("Polarity inverted.".to_string());
                }
            }
            "delay" => {
                let samples =
                    if let Some(samples) = part.fields.get("samples").and_then(Value::as_u64) {
                        samples as usize
                    } else {
                        let seconds = part
                            .fields
                            .get("delay_s")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0);
                        (seconds * sample_rate_hz).round().max(0.0) as usize
                    };
                *delay += samples;
                lines.push(format!("Delay of {samples} samples."));
            }
            other => lines.push(format!("A {other} block was left out of playback.")),
        }
    }
}

fn impulse_responses(
    design: &DesignFile,
    registry: &Registry,
    sample_rate_hz: f64,
    n: usize,
) -> Vec<Vec<f64>> {
    let n = n.next_power_of_two().clamp(32, 4096);
    let mut freqs = Vec::with_capacity(n / 2 + 1);
    freqs.push(1.0);
    for bin in 1..=n / 2 {
        freqs.push(bin as f64 * sample_rate_hz / n as f64);
    }
    // The impulse is from amplifier voltage to pressure. The DSP chain runs in the
    // real-time kernel, so it is not part of this response.
    let mut acoustic = design.clone();
    acoustic.dsp.whole_speaker.clear();
    for chain in &mut acoustic.dsp.per_radiator {
        chain.blocks.clear();
        chain.gain_db = 0.0;
        chain.delay_s = 0.0;
        chain.invert = false;
    }
    let result = simulate(
        &acoustic,
        &SimulateRequest {
            voltage_v: 1.0,
            frequencies_hz: Some(freqs),
            ..SimulateRequest::default()
        },
        registry,
    );
    let mut planner = rustfft::FftPlanner::<f64>::new();
    let ifft = planner.plan_fft_inverse(n);
    result
        .radiators
        .iter()
        .map(|curve| {
            let mut spectrum = vec![num_complex::Complex64::new(0.0, 0.0); n];
            for bin in 1..n / 2 {
                let re = curve.pressure.re.get(bin).copied().unwrap_or(0.0);
                let im = curve.pressure.im.get(bin).copied().unwrap_or(0.0);
                if !re.is_finite() || !im.is_finite() {
                    continue;
                }
                spectrum[bin] = num_complex::Complex64::new(re, im);
                spectrum[n - bin] = num_complex::Complex64::new(re, -im);
            }
            if let Some(re) = curve.pressure.re.get(n / 2).copied() {
                if re.is_finite() {
                    spectrum[n / 2] = num_complex::Complex64::new(re, 0.0);
                }
            }
            ifft.process(&mut spectrum);
            spectrum
                .iter()
                .map(|sample| {
                    let value = sample.re / n as f64;
                    if value.is_finite() {
                        value
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

pub fn simulate_json(text: &str, request_json: &str, registry: &Registry) -> Result<Value, String> {
    let design: DesignFile = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let request: SimulateRequest = if request_json.trim().is_empty() {
        SimulateRequest::default()
    } else {
        serde_json::from_str(request_json).map_err(|err| err.to_string())?
    };
    let result = simulate(&design, &request, registry);
    serde_json::to_value(result).map_err(|err| err.to_string())
}
