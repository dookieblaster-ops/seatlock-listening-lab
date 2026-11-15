//! Turns a design into simulation results, score, cost, and optimizer output.

use std::time::Instant;

use num_complex::Complex64;
use seatlock_acoustics::{
    baffle_step_hz_for_outline, blend_measured_curve, cone_displacement_m3, solve, spl_db,
    vented_sample, PortSpec,
};
use seatlock_core::catalog;
use seatlock_core::constants::{C0, P_REF, RHO0};
use seatlock_core::design::{DesignFile, KindPart};
use seatlock_core::grid::default_grid;
use seatlock_core::issue::Issue;
use seatlock_core::registry::{
    DspBlock, OuterShape, RadiationScene, RadiatorEval, Registry, RoomSound, SourcePatch,
};
use seatlock_correct::metrics;
use seatlock_dsp::{delay_block, gain_block, polarity_block, prefer_inverted_high, ThiranDelay};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub mod chambers;
pub mod compare;
pub mod cost;
pub mod exterior;
pub mod generate;
pub mod optimize;
pub mod ports;
pub mod shape;
pub mod solids;
pub mod split;
pub mod voxels;
pub mod walls;
pub use cost::{CabinetParts, CostLine, CostReport};

pub fn builtin() -> Registry {
    let mut registry = Registry::new();
    seatlock_geom::register(&mut registry);
    seatlock_acoustics::register(&mut registry);
    seatlock_dsp::register(&mut registry);
    registry.register_radiation("exterior", exterior::parse);
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
    /// Cell size of the full 3D model when `enclosure` is `fdtd`; 10 mm by default.
    #[serde(default)]
    pub fdtd_dx_m: Option<f64>,
    /// Also return sound power and the directivity index (a room asks for them itself).
    #[serde(default)]
    pub sound_power: bool,
    /// Also compute the level a little off to the sides and above and below (the listening
    /// window of README section 11.1), and score evenness on the worse of it and the level
    /// straight ahead.
    #[serde(default)]
    pub listening_window: bool,
    /// Include the air's losses at the walls (README sections 5.4 and 5.5): in the detailed
    /// model's ducts, and in the 3D model as one average over the cabinet.
    #[serde(default)]
    pub losses: bool,
    /// Let the cabinet's walls vibrate (README section 5.11): each flat wall of a printed
    /// cabinet is a plate of the wall build's layers, pushed by the pressure inside, and it
    /// sends out sound of its own.
    #[serde(default)]
    pub walls: bool,
    /// Cell size of the air outside the cabinet when `radiation` is `exterior`; 10 mm by
    /// default.
    #[serde(default)]
    pub exterior_dx_m: Option<f64>,
    /// Run the 3D cabinet model until its echoes die away instead of stopping early and
    /// fitting the rest (README 5.5). For checks that compare the two.
    #[serde(default)]
    pub fdtd_full_decay: bool,
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
            fdtd_dx_m: None,
            sound_power: false,
            listening_window: false,
            losses: false,
            walls: false,
            exterior_dx_m: None,
            fdtd_full_decay: false,
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
    /// Dollars for the whole system, line by line (README section 9.2).
    pub cost: CostReport,
    pub elapsed_ms: f64,
    pub f6_hz: f64,
    pub flatness_db: f64,
    pub max_spl_metric_db: f64,
    /// What the full 3D model did, when it ran.
    pub fdtd: Option<FdtdReport>,
    /// Sound power level, dB re 1 pW, when the room or the request asked for it.
    pub sound_power_db: Option<Vec<f64>>,
    /// The listening window's level (README section 11.1), when the request asked for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listening_window_db: Option<Vec<f64>>,
    /// Evenness of the level straight ahead (in the room, when the design has one), and of
    /// the listening window; `flatness_db` is the worse of the two.
    #[serde(default)]
    pub axis_flatness_db: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_flatness_db: Option<f64>,
    /// On-axis level against the mean over all directions, in dB.
    pub directivity_index_db: Option<Vec<f64>>,
    /// The in-room response, when the design has a room (README section 5.10). The score
    /// then comes from it; `summed_spl_db` stays the direct sound.
    pub in_room: Option<InRoomReport>,
}

/// The sound at the listening position in a room, in dB SPL at each frequency.
#[derive(Debug, Clone, Serialize)]
pub struct InRoomReport {
    /// Direct sound and reflections added as pressures, diffuse sound added as energy.
    pub spl_db: Vec<f64>,
    /// Direct sound plus reflections.
    pub early_spl_db: Vec<f64>,
    pub reflection_spl_db: Vec<f64>,
    pub diffuse_spl_db: Vec<f64>,
}

/// One run of the full 3D model for a builder cabinet (README section 5.5).
#[derive(Debug, Clone, Serialize)]
pub struct FdtdReport {
    pub dx_m: f64,
    pub cells: usize,
    pub steps: usize,
    pub decayed: bool,
    pub elapsed_ms: f64,
    /// Above this the grid is too coarse (`C0 / (10 dx)`), and the detailed model is used.
    pub valid_up_to_hz: f64,
    /// Inside air on the grid, and the net air volume the quick model uses.
    pub grid_volume_m3: f64,
    pub net_volume_m3: f64,
    /// Largest difference from the quick model's `Z_ab` at low frequency (the self-check).
    pub self_check_error: Option<f64>,
    pub frequencies_hz: Vec<f64>,
    /// `Z_ab` from the 3D model, and from the quick model at the same frequencies.
    pub z_ab: ComplexCurve,
    pub z_ab_quick: ComplexCurve,
    pub port_ids: Vec<String>,
}

pub fn simulate(
    design: &DesignFile,
    request: &SimulateRequest,
    registry: &Registry,
) -> SimulateResult {
    simulate_with_progress(design, request, registry, &mut |_, _| true)
}

/// [`simulate`], with `progress(steps, max_steps)` called during a full 3D run. Returning false
/// stops the run, and the result then comes from the detailed model.
pub fn simulate_with_progress(
    design: &DesignFile,
    request: &SimulateRequest,
    registry: &Registry,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> SimulateResult {
    let full_3d = request.enclosure == "fdtd";
    // The full 3D model refines the detailed model's curves up to its grid's limit.
    let base = if full_3d {
        "waveguide"
    } else {
        request.enclosure.as_str()
    };
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

    let mut ignored = vec!["no cone break-up".to_string()];
    if base == "lumped" {
        ignored.push("the quick model ignores cabinet shape and standing waves".to_string());
    } else if base == "waveguide" && !full_3d && design.speaker.enclosure.ducts.is_empty() {
        ignored.push(
            "the detailed model treats a box without ducts as one compliance, so standing \
             waves inside it are not modeled"
                .to_string(),
        );
    }
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
    let faces_listener = design
        .speaker
        .enclosure
        .mounts
        .iter()
        .all(|mount| mount.surface == "front");
    if !faces_listener && request.radiation == "point" {
        issues.push(Issue::warning(
            "unsupported_kind",
            "A sound source does not face the listener. This quick model treats it as if it did.",
            "speaker.enclosure.mounts",
        ));
    }

    issues.extend(breakup_warnings(design));

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
        cost: design_cost(design),
        elapsed_ms: elapsed_ms(started),
        f6_hz: 0.0,
        flatness_db: 0.0,
        max_spl_metric_db: 0.0,
        fdtd: None,
        sound_power_db: None,
        listening_window_db: None,
        axis_flatness_db: 0.0,
        window_flatness_db: None,
        directivity_index_db: None,
        in_room: None,
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
    if !outer.moulded_openings().is_empty() {
        fidelity
            .ignored
            .retain(|line| !line.starts_with("the detailed model treats a box without ducts"));
    }
    let baffle = outer.baffle_width_m();
    if let Err(err) = outer.outer_size_m() {
        issues.push(Issue::error(
            "unsupported_kind",
            shape_message(&design.speaker.enclosure.outer.kind, &err),
            "speaker.enclosure.outer",
        ));
        return fail(issues, fidelity);
    }
    for mount in &design.speaker.enclosure.mounts {
        if let Err(err) = outer.mount_pose(&mount.surface, mount.at_m) {
            issues.push(Issue::error(
                "bad_mount",
                format!(
                    "Mount {}: {}",
                    mount.id,
                    shape_message(&design.speaker.enclosure.outer.kind, &err)
                ),
                "speaker.enclosure.mounts",
            ));
            return fail(issues, fidelity);
        }
    }

    let enclosure = match registry.create_enclosure(base, &Map::new()) {
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
    if outer.part_materials().iter().any(|part| part.3) && !full_3d {
        fidelity.ignored.push(
            "Damping parts are modeled only by the full 3D model; this model treats them as \
             air."
                .into(),
        );
    }
    // Loads of chambers other than the first driver's, by chamber index, and for each
    // radiator which load it sees: 0 for the first driver's chamber, n + 1 for chamber n.
    let mut other_nodes: Vec<(
        usize,
        Vec<Complex64>,
        Option<Vec<seatlock_acoustics::VentedSample>>,
    )> = Vec::new();
    let mut radiator_nodes: Vec<Option<usize>> = Vec::new();
    // Walls of a cabinet divided into chambers (README 5.11): each chamber's walls are a
    // shunt in the network, and every chamber's pressure per unit volume velocity pushed in
    // is kept per load (the same keys as `radiator_nodes`), so the walls can be sources.
    let mut chamber_cabinet: Option<Result<walls::CabinetWalls, String>> = None;
    let mut driver_chamber = 0usize;
    let mut node_pressures: std::collections::HashMap<usize, Vec<Vec<Complex64>>> =
        Default::default();
    let network = chambers::air_network(
        design,
        outer.as_ref(),
        |part| {
            if part.kind == "cone" {
                cone_displacement_m3(&part.fields).0
            } else {
                0.0
            }
        },
        driver_loads_box,
    );
    let (volume, ports, z_ab, vented) = match network {
        Some(Err(err)) => {
            issues.push(Issue::error(
                "bad_chamber",
                err,
                "speaker.enclosure.chambers",
            ));
            return fail(issues, fidelity);
        }
        Some(Ok(net)) => {
            let wall_y: Vec<Vec<Complex64>> = if request.walls {
                let modeled = walls::chamber_walls(design, outer.as_ref(), &net.chambers);
                let y = match &modeled {
                    Ok(cabinet) => freqs
                        .iter()
                        .map(|f| {
                            (0..net.chambers.len())
                                .map(|c| cabinet.chamber_admittance(c, *f))
                                .collect()
                        })
                        .collect(),
                    Err(_) => vec![Vec::new(); freqs.len()],
                };
                chamber_cabinet = Some(modeled);
                y
            } else {
                vec![Vec::new(); freqs.len()]
            };
            // The load each chamber a driver pushes into presents: the first driver's first,
            // then any other chamber a driver pushes into (README 5.2). Chambers that are not
            // joined by a duct do not affect each other, so each is exact on its own.
            let solve_from = |node: usize, fidelity: &mut Fidelity, notes: bool| {
                let mut impedance = Vec::with_capacity(freqs.len());
                let mut samples = Vec::with_capacity(freqs.len());
                let mut pressures = Vec::with_capacity(freqs.len());
                for (bin, freq) in freqs.iter().enumerate() {
                    let solved = seatlock_acoustics::network::lumped_network_with_walls(
                        &net.volumes_m3,
                        &net.ducts,
                        node,
                        *freq,
                        &wall_y[bin],
                    );
                    pressures.push(solved.pressures.clone());
                    let mut sample = seatlock_acoustics::VentedSample {
                        z_ab: solved.z_ab,
                        h_sum: Complex64::new(0.0, 0.0),
                        h_each: [Complex64::new(0.0, 0.0); 4],
                        port_count: solved.h_out.len().min(4),
                    };
                    for (index, h) in solved.h_out.iter().take(4).enumerate() {
                        sample.h_each[index] = *h;
                        sample.h_sum += *h;
                    }
                    impedance.push(sample.z_ab);
                    samples.push(sample);
                }
                let names: Vec<String> = net.chambers.iter().map(|c| c.id.clone()).collect();
                let wave = if base == "waveguide" {
                    Some(chambers::waveguide_network(
                        design,
                        &names,
                        &net.volumes_m3,
                        node,
                        &net.moulded,
                    ))
                } else {
                    None
                };
                match wave {
                    Some(Ok(network)) => {
                        for (bin, freq) in freqs.iter().enumerate() {
                            let solved = seatlock_acoustics::solve_with_walls(
                                &network.root,
                                *freq,
                                request.losses,
                                &wall_y[bin],
                            );
                            for (slot, p) in &solved.pressures {
                                if let Some(cell) = pressures[bin].get_mut(*slot) {
                                    *cell = *p;
                                }
                            }
                            let sample = &mut samples[bin];
                            sample.z_ab = solved.z_in;
                            sample.h_each = [Complex64::new(0.0, 0.0); 4];
                            sample.h_sum = Complex64::new(0.0, 0.0);
                            for (slot, h) in network.opening_order.iter().zip(&solved.openings) {
                                if *slot < 4 {
                                    sample.h_each[*slot] = *h;
                                    sample.h_sum += *h;
                                }
                            }
                            impedance[bin] = solved.z_in;
                        }
                        if notes {
                            fidelity.ignored.extend(network.notes);
                        }
                    }
                    Some(Err(err)) => {
                        fidelity.ignored.push(format!(
                            "The detailed model cannot solve this cabinet ({err}), so these \
                             curves come from the quick model. Several chambers: each is one \
                             compliance and each duct one air mass."
                        ));
                    }
                    None if notes => fidelity.ignored.push(
                        "Several chambers: each is one compliance and each duct one air mass, \
                         so standing waves and pipe resonances are not modeled."
                            .into(),
                    ),
                    None => {}
                }
                (impedance, samples, pressures)
            };
            fidelity.ignored.extend(net.notes.iter().cloned());
            let (impedance, samples, pressures) = solve_from(net.driver, &mut fidelity, true);
            node_pressures.insert(0, pressures);
            driver_chamber = net.driver;
            for node in net.driver_nodes.iter().flatten() {
                if *node != net.driver && !other_nodes.iter().any(|(n, _, _)| n == node) {
                    let (z, s, p) = solve_from(*node, &mut fidelity, false);
                    node_pressures.insert(node + 1, p);
                    other_nodes.push((*node, z, if net.ports.is_empty() { None } else { Some(s) }));
                }
            }
            if !other_nodes.is_empty() {
                fidelity.ignored.push(
                    "Drivers push into different chambers; each sees the load of its own \
                     chamber, and chambers joined by a duct are each solved as if the other's \
                     driver were still."
                        .into(),
                );
            }
            radiator_nodes = net
                .driver_nodes
                .iter()
                .map(|node| node.map(|n| if n == net.driver { 0 } else { n + 1 }))
                .collect();
            let total = net.volumes_m3.iter().sum::<f64>();
            let vented = if net.ports.is_empty() {
                None
            } else {
                Some(samples)
            };
            (total, net.ports, impedance, vented)
        }
        None => {
            let mut displacement = 0.0;
            for radiator in &design.speaker.radiators {
                if radiator.kind == "cone" {
                    let (amount, _, _) = cone_displacement_m3(&radiator.fields);
                    displacement += amount;
                }
            }
            // A printed port's tube takes air from the chamber (README section 5.3).
            for port in ports::printed_ports(design, outer.as_ref(), &[])
                .into_iter()
                .flatten()
            {
                displacement += port.displaced_m3;
            }
            // Plastic placed in the air takes its volume too.
            if let Ok(placed) = solids::solids(design) {
                let in_air = |p: [f64; 3]| outer.contains_air(p).unwrap_or(false);
                displacement += placed.plastic_m3(&in_air);
            }
            let volume = (gross - displacement).max(0.0);
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
            let wave = if base == "waveguide" && !ports.is_empty() {
                Some(chambers::waveguide_network(
                    design,
                    &["main".to_string()],
                    &[volume],
                    0,
                    &[],
                ))
            } else {
                None
            };
            let (z_ab, vented) = match wave {
                Some(Ok(network)) => {
                    let mut impedance = Vec::with_capacity(freqs.len());
                    let mut samples = Vec::with_capacity(freqs.len());
                    for freq in &freqs {
                        let wave = if request.losses {
                            seatlock_acoustics::solve_lossy(&network.root, *freq)
                        } else {
                            solve(&network.root, *freq)
                        };
                        let mut sample = seatlock_acoustics::VentedSample {
                            z_ab: wave.z_in,
                            h_sum: Complex64::new(0.0, 0.0),
                            h_each: [Complex64::new(0.0, 0.0); 4],
                            port_count: ports.len().min(4),
                        };
                        for (slot, h) in network.opening_order.iter().zip(&wave.openings) {
                            if *slot < 4 {
                                sample.h_each[*slot] = *h;
                                sample.h_sum += *h;
                            }
                        }
                        impedance.push(sample.z_ab);
                        samples.push(sample);
                    }
                    fidelity.ignored.extend(network.notes);
                    (impedance, Some(samples))
                }
                Some(Err(err)) => {
                    fidelity.ignored.push(format!(
                        "The detailed model cannot solve this cabinet ({err}), so these curves \
                         come from the quick model."
                    ));
                    (z_ab, vented)
                }
                None => (z_ab, vented),
            };
            (volume, ports, z_ab, vented)
        }
    };
    let (mut z_ab, mut vented) = (z_ab, vented);
    let mut fdtd_report = None;
    // Frequencies whose `Z_ab` the full 3D model replaced (its grid's walls are rigid).
    let mut replaced_3d = vec![false; freqs.len()];
    if full_3d {
        match full_3d_enclosure(
            design,
            outer.as_ref(),
            &freqs,
            volume,
            &z_ab,
            request.fdtd_dx_m.unwrap_or(0.01),
            request.fdtd_full_decay,
            request.losses,
            &driver_impedance_scale(design, registry, &freqs, &z_ab),
            progress,
        ) {
            Ok(run) => {
                for (bin, z) in run.z.iter().enumerate() {
                    if let Some(z) = z {
                        z_ab[bin] = *z;
                        replaced_3d[bin] = true;
                    }
                }
                if let Some(samples) = vented.as_mut() {
                    if run.h.len() == ports.len() {
                        for (bin, sample) in samples.iter_mut().enumerate() {
                            if run.z[bin].is_none() {
                                continue;
                            }
                            sample.z_ab = z_ab[bin];
                            sample.h_sum = Complex64::new(0.0, 0.0);
                            for (port, h) in run.h.iter().enumerate().take(4) {
                                sample.h_each[port] = h[bin];
                                sample.h_sum += h[bin];
                            }
                        }
                    } else {
                        fidelity.ignored.push(
                            "The ports' flows come from the detailed model: the 3D grid's \
                             openings did not match the ports one to one."
                                .into(),
                        );
                    }
                }
                if let Some(error) = run.report.self_check_error {
                    if error > 0.03 {
                        let ports_note = if ports.is_empty() {
                            ""
                        } else {
                            " With ports, part of the difference is the ports' end corrections \
                             and flares, which the detailed model only estimates."
                        };
                        issues.push(Issue::warning(
                            "fdtd_self_check",
                            format!(
                                "At low frequency the full 3D air load differs from the detailed \
                                 model by {:.1}% (it should be under 3%). The 3D grid holds \
                                 {:.3} L of air and the design {:.3} L.{ports_note}",
                                error * 100.0,
                                run.report.grid_volume_m3 * 1000.0,
                                run.report.net_volume_m3 * 1000.0
                            ),
                            "speaker.enclosure",
                        ));
                    }
                }
                if !run.report.decayed {
                    issues.push(Issue::warning(
                        "fdtd_not_decayed",
                        "The full 3D run reached its time limit before the sound died away, so \
                         values near sharp resonances are less certain.",
                        "speaker.enclosure",
                    ));
                }
                // The quick and detailed models' own limits apply only above the grid's limit.
                fidelity.ignored.retain(|line| {
                    ![
                        "sealed-box model",
                        "left out of the sealed box",
                        "quick model ignores",
                        "Bends are treated",
                        "Several chambers: each is one compliance",
                        "does not solve several chambers",
                        "in the ducts treated as straight",
                        "Chambers are single compliances",
                        "treats a box without ducts",
                    ]
                    .iter()
                    .any(|phrase| line.contains(phrase))
                });
                fidelity.ignored.push(format!(
                    "Full 3D at {:.0} mm cells up to {:.0} Hz; above that the detailed model.",
                    run.report.dx_m * 1000.0,
                    run.report.valid_up_to_hz
                ));
                fidelity
                    .ignored
                    .push("Full 3D: rigid walls and air without losses.".into());
                fidelity
                    .ignored
                    .push(seatlock_fdtd::PEAK_HEIGHTS_NOT_MODELED.into());
                fidelity.ignored.extend(run.notes);
                fdtd_report = Some(run.report);
            }
            Err(err) => {
                fidelity.enclosure = base.to_string();
                issues.push(Issue::warning(
                    "fdtd_failed",
                    format!(
                        "The full 3D model could not run: {err} These curves come from the \
                         detailed model."
                    ),
                    "speaker.enclosure",
                ));
            }
        }
    }
    // Vibrating walls (README section 5.11): a shunt on the driver's chamber, and sources of
    // their own. All flows in the cabinet follow the chamber's pressure, so the shunt scales
    // `Z_ab` and every port's `H_k` alike.
    let mut wall_admittances: Vec<Vec<Complex64>> = Vec::new();
    let mut wall_patches: Vec<SourcePatch> = Vec::new();
    let mut wall_chambers: Vec<usize> = Vec::new();
    let wall_build = design
        .build
        .extra
        .get("wall")
        .and_then(|wall| wall.get("kind"))
        .and_then(Value::as_str)
        .filter(|kind| *kind != "solid");
    if request.walls {
        let divided = chamber_cabinet.is_some();
        let modeled = match chamber_cabinet.take() {
            Some(result) => result,
            None => walls::cabinet_walls(design, outer.as_ref()),
        };
        match modeled {
            Ok(cabinet) if divided => {
                // Already a shunt on every chamber in the network; only the full 3D model's
                // `Z_ab`, from rigid walls, takes its own chamber's walls here.
                for (bin, freq) in freqs.iter().enumerate() {
                    if replaced_3d[bin] {
                        let scale = Complex64::new(1.0, 0.0)
                            / (Complex64::new(1.0, 0.0)
                                + z_ab[bin] * cabinet.chamber_admittance(driver_chamber, *freq));
                        z_ab[bin] *= scale;
                        if let Some(sample) = vented.as_mut().and_then(|s| s.get_mut(bin)) {
                            sample.z_ab = z_ab[bin];
                            sample.h_sum *= scale;
                            for h in sample.h_each.iter_mut() {
                                *h *= scale;
                            }
                        }
                        if let Some(cell) = node_pressures
                            .get_mut(&0)
                            .and_then(|p| p.get_mut(bin))
                            .and_then(|p| p.get_mut(driver_chamber))
                        {
                            *cell = z_ab[bin];
                        }
                    }
                    wall_admittances.push(cabinet.element_admittances(*freq));
                }
                if replaced_3d.iter().any(|r| *r) {
                    fidelity.ignored.push(
                        "Full 3D: the driver's chamber's walls are added to its result; the \
                         other chambers' pressures on their walls come from the detailed model."
                            .into(),
                    );
                }
                wall_patches = cabinet.patches();
                wall_chambers = cabinet.element_chambers();
                fidelity.ignored.extend(cabinet.notes.iter().cloned());
            }
            Ok(cabinet) => {
                for (bin, freq) in freqs.iter().enumerate() {
                    let scale = Complex64::new(1.0, 0.0)
                        / (Complex64::new(1.0, 0.0) + z_ab[bin] * cabinet.admittance(*freq));
                    z_ab[bin] *= scale;
                    if let Some(sample) = vented.as_mut().and_then(|s| s.get_mut(bin)) {
                        sample.z_ab = z_ab[bin];
                        sample.h_sum *= scale;
                        for h in sample.h_each.iter_mut() {
                            *h *= scale;
                        }
                    }
                    wall_admittances.push(cabinet.element_admittances(*freq));
                }
                wall_patches = cabinet.patches();
                wall_chambers = cabinet.element_chambers();
                fidelity.ignored.extend(cabinet.notes.iter().cloned());
            }
            Err(why) => {
                fidelity.ignored.push(format!("Walls are rigid: {why}."));
                if wall_build.is_some() {
                    issues.push(Issue::warning(
                        "wall_build_not_heard",
                        format!(
                            "The wall build changes the cost and the weight, but the sound \
                             cannot show it here: {why}."
                        ),
                        "build.wall",
                    ));
                }
            }
        }
    } else if let Some(kind) = wall_build {
        issues.push(Issue::warning(
            "wall_build_not_heard",
            format!(
                "The {} wall build changes the cost and the weight; these curves were asked for \
                 with rigid walls, so the sound does not show it (README section 5.11).",
                if kind == "filled" {
                    "filled"
                } else {
                    "two-material"
                }
            ),
            "build.wall",
        ));
    }
    let silent = vec![Complex64::new(0.0, 0.0); freqs.len()];
    let mut radiation_fields = Map::new();
    if let Some(dx) = request.exterior_dx_m {
        radiation_fields.insert("dx_m".into(), serde_json::json!(dx));
    }
    let radiation = match registry.create_radiation(&request.radiation, &radiation_fields) {
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
            issues.push(Issue::warning(
                "unsupported_kind",
                format!("{err} The results are for the speaker without a room."),
                "speaker.room",
            ));
            fidelity.room = "free".to_string();
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

    // The listener sits straight in front of the middle of the cabinet's front.
    let size = outer.outer_size_m().unwrap_or([0.0; 3]);
    let reference = [size[0] / 2.0, 0.0, size[2] / 2.0];
    let listener = [
        size[0] / 2.0,
        -design.speaker.listening.distance_m,
        size[2] / 2.0,
    ];
    let scene = RadiationScene {
        shape: outer.as_ref(),
        space: &request.space,
        baffle_width_m: baffle,
        reference_m: reference,
    };

    // Every radiator at 1 V: its DSP chain and its surface sources (README section 5.6).
    struct Prepared<'a> {
        index: usize,
        part: &'a KindPart,
        eval: RadiatorEval,
        blocks_h: Vec<Complex64>,
        /// Volume velocity per volt of each surface source.
        sources: Vec<Vec<Complex64>>,
        patches: Vec<SourcePatch>,
        port_speed_1v: Vec<f64>,
        /// True for a radiator that gave its own grid of surface sources.
        is_surface: bool,
    }
    // Drivers that push into the same chamber share its air: driven alike, each sees the
    // chamber's load times their number (README 5.2).
    let mut sharing_counts: std::collections::HashMap<usize, usize> = Default::default();
    let mut sharing_chains: std::collections::HashMap<usize, Vec<String>> = Default::default();
    for (index, part) in design.speaker.radiators.iter().enumerate() {
        let loads = registry
            .create_radiator(&part.kind, &part.fields)
            .map(|r| r.loads_box())
            .unwrap_or(false)
            && driver_loads_box(part);
        if !loads {
            continue;
        }
        let node = radiator_nodes.get(index).copied().flatten().unwrap_or(0);
        *sharing_counts.entry(node).or_insert(0) += 1;
        let chain = design
            .dsp
            .per_radiator
            .get(index)
            .map(|c| {
                serde_json::to_string(&(&c.blocks, c.gain_db, c.delay_s, c.invert))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        sharing_chains.entry(node).or_default().push(format!(
            "{}|{chain}",
            part.fields
                .get("driver_id")
                .map(|v| v.to_string())
                .unwrap_or_default()
        ));
    }
    if sharing_counts.values().any(|count| *count > 1) {
        let alike = sharing_chains
            .values()
            .all(|list| list.iter().all(|item| item == &list[0]));
        fidelity.ignored.push(if alike {
            "Drivers that share a chamber are the same model with the same processing, so each \
             sees the chamber's load times their number."
                .to_string()
        } else {
            "Drivers that share a chamber are taken to move alike, each seeing the chamber's \
             load times their number; they differ in model or processing, so this is an \
             approximation where their outputs differ."
                .to_string()
        });
    }
    let mut prepared: Vec<Prepared> = Vec::new();
    for (index, part) in design.speaker.radiators.iter().enumerate() {
        let radiator = match registry.create_radiator(&part.kind, &part.fields) {
            Ok(radiator) => radiator,
            Err(err) => {
                issues.push(Issue::warning(
                    "missing_field",
                    err,
                    format!("speaker.radiators[{index}]"),
                ));
                continue;
            }
        };
        let loads_box = radiator.loads_box() && driver_loads_box(part);
        // The load of the chamber this driver pushes into, shared with the other drivers that
        // push into it (README 5.2).
        let node = radiator_nodes.get(index).copied().flatten().unwrap_or(0);
        let (node_z, node_vented) = match other_nodes.iter().find(|(n, _, _)| n + 1 == node) {
            Some((_, z, v)) => (z, v.as_ref()),
            None => (&z_ab, vented.as_ref()),
        };
        let sharing = sharing_counts.get(&node).copied().unwrap_or(1).max(1);
        let shared_z: Vec<Complex64>;
        let node_z = if sharing > 1 && loads_box {
            shared_z = node_z.iter().map(|z| z * sharing as f64).collect();
            &shared_z
        } else {
            node_z
        };
        let mut eval = radiator.evaluate(&freqs, if loads_box { node_z } else { &silent });
        issues.append(&mut eval.warnings);
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
            let samples = (chain.delay_s + geometric_delay_s(design, index)) * sample_rate;
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
            if let Some(samples) = node_vented {
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
        // A source the shape cannot place has no direction, and every model then treats it
        // as a simple source at the middle of the front.
        let unplaced = SourcePatch {
            position_m: reference,
            outward: [0.0; 3],
            area_m2: 0.0,
        };
        let surface = radiator.surface(&freqs);
        let is_surface = surface.is_some();
        let mut patches: Vec<SourcePatch> = if let Some(elements) = surface {
            // A radiator that is a grid of sources: each sits on the mount's surface at its
            // own surface coordinates (README sections 5.6 and 5.9).
            let mount = part
                .fields
                .get("mount")
                .and_then(Value::as_str)
                .and_then(|id| {
                    design
                        .speaker
                        .enclosure
                        .mounts
                        .iter()
                        .find(|mount| mount.id == id)
                });
            let mut off_surface: Option<String> = None;
            let mut patches = Vec::with_capacity(elements.len());
            // The surface's own outline, when it is flat: a source beyond it is in thin air.
            let outline = mount.and_then(|mount| outer.surface_outline(&mount.surface).ok());
            sources.clear();
            for element in elements {
                let pose = mount
                    .ok_or_else(|| "it names no mount.".to_string())
                    .and_then(|mount| {
                        outer.mount_pose(
                            &mount.surface,
                            [
                                mount.at_m[0] + element.at_m[0],
                                mount.at_m[1] + element.at_m[1],
                            ],
                        )
                    });
                let pose = pose.and_then(|pose| match &outline {
                    Some(corners) if !inside_outline(pose.position_m, corners, pose.outward) => {
                        Err("it reaches past the edge of that surface.".to_string())
                    }
                    _ => Ok(pose),
                });
                patches.push(match pose {
                    Ok(pose) => SourcePatch {
                        position_m: pose.position_m,
                        outward: pose.outward,
                        area_m2: element.area_m2,
                    },
                    Err(err) => {
                        off_surface.get_or_insert(err);
                        unplaced
                    }
                });
                sources.push(element.volume_velocity);
            }
            if let Some(err) = off_surface {
                issues.push(Issue::warning(
                    "panel_off_surface",
                    format!(
                        "Part of sound source {index} is not on its mounting surface, so that \
                         part is treated as a simple source: {err}"
                    ),
                    format!("speaker.radiators[{index}].mount"),
                ));
            }
            patches
        } else {
            let placed = placed_sources(design, outer.as_ref(), part, sources.len());
            (0..sources.len())
                .map(|slot| match placed.get(slot) {
                    Some(place) => SourcePatch {
                        position_m: place.position,
                        outward: place.outward,
                        area_m2: std::f64::consts::PI * place.radius_m * place.radius_m,
                    },
                    None => unplaced,
                })
                .collect()
        };
        // The walls this driver pushes on: each element moves `Y_e * p`, with its chamber's
        // pressure `p = P * U_in` and `U_in = -U_d` (README 5.11). `P` is `Z_ab` when the
        // inside is one chamber, and that chamber's pressure per unit pushed in otherwise.
        if loads_box && !wall_admittances.is_empty() {
            let pressures = node_pressures.get(&node);
            for (element, patch) in wall_patches.iter().enumerate() {
                let chamber = wall_chambers.get(element).copied().unwrap_or(0);
                sources.push(
                    (0..freqs.len())
                        .map(|bin| {
                            let p = match pressures {
                                Some(p) => p[bin].get(chamber).copied().unwrap_or_default(),
                                None => z_ab[bin],
                            };
                            -wall_admittances[bin][element] * p * eval.volume_velocity[bin]
                        })
                        .collect(),
                );
                patches.push(*patch);
            }
        }
        for note in radiator.notes() {
            if !fidelity.ignored.contains(&note) {
                fidelity.ignored.push(note);
            }
        }
        prepared.push(Prepared {
            index,
            part,
            eval,
            blocks_h,
            sources,
            patches,
            port_speed_1v,
            is_surface,
        });
    }

    // One call for all sources: a 3D radiation model then runs once for the listening position.
    let all_patches: Vec<SourcePatch> = prepared
        .iter()
        .flat_map(|item| item.patches.iter().copied())
        .collect();
    let direct = match radiation.transfer(&scene, &all_patches, &freqs, listener, &mut *progress) {
        Ok(transfer) => transfer,
        Err(err) => {
            issues.push(Issue::error(
                "radiation_failed",
                format!(
                    "The {} radiation model could not run: {err}",
                    request.radiation
                ),
                "request.radiation",
            ));
            return fail(issues, fidelity);
        }
    };
    let add_notes = |fidelity: &mut Fidelity, notes: &[String]| {
        for note in notes {
            if !fidelity.ignored.contains(note) {
                fidelity.ignored.push(note.clone());
            }
        }
    };
    add_notes(&mut fidelity, &direct.notes);
    if direct
        .notes
        .iter()
        .any(|note| note == seatlock_acoustics::SHADOW_NOTE)
    {
        issues.push(Issue::warning(
            "source_faces_away",
            "A sound source faces away from the listener. This everyday model only estimates \
             what bends around its panel; run the full 3D outside model to judge such a design.",
            "speaker.enclosure.mounts",
        ));
    }

    let mut radiators = Vec::new();
    let mut input_limit = vec![f64::MAX; freqs.len()];
    let mut any_measured = false;
    let mut first_source = 0usize;
    for item in &prepared {
        let rows = &direct.per_source[first_source..first_source + item.sources.len()];
        first_source += item.sources.len();
        let (index, part, eval) = (item.index, item.part, &item.eval);
        let (blocks_h, port_speed_1v) = (&item.blocks_h, &item.port_speed_1v);
        let mut pressure_1v: Vec<Complex64> = (0..freqs.len())
            .map(|bin| {
                item.sources
                    .iter()
                    .zip(rows)
                    .map(|(velocity, row)| velocity[bin] * row[bin])
                    .sum()
            })
            .collect();
        // A cone's published response curve replaces the piston model above its break-up; a
        // radiator with its own grid of sources has no such curve.
        if !item.is_surface {
            let measured = measured_curve(part);
            any_measured |= !measured.is_empty();
            let label =
                blend_measured_curve(&mut pressure_1v, &freqs, piston_radius_m(part), &measured);
            if !fidelity.ignored.iter().any(|line| line == &label) {
                fidelity.ignored.push(label);
            }
        }
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
            // The input level at which this radiator reaches its first limit, through its
            // own DSP chain (the system's maximum level comes from the lowest of these).
            let volts_at_limit = 10f64.powf((clean - spl_db(pressure_1v[i])) / 20.0);
            input_limit[i] = input_limit[i].min(volts_at_limit / blocks_h[i].norm().max(1e-12));
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

    // Which radiator, by its place in `radiators`, the automatic polarity flipped.
    let mut flipped: Option<usize> = None;
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
                flipped = Some(index);
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

    // The room (README section 5.10) and the sound power it rests on (section 5.6), from every
    // source as it is driven: 1 V response, DSP chain, request voltage, and polarity.
    let mut in_room: Option<InRoomReport> = None;
    let mut sound_power_db: Option<Vec<f64>> = None;
    let mut directivity_index_db: Option<Vec<f64>> = None;
    let images = room.images(listener, size);
    let wants_power = room.needs_sound_power() || request.sound_power;
    if !prepared.is_empty() && (!images.is_empty() || wants_power) {
        let mut driven: Vec<Vec<Complex64>> = Vec::with_capacity(all_patches.len());
        for (place, item) in prepared.iter().enumerate() {
            let sign = if flipped == Some(place) { -1.0 } else { 1.0 };
            for source in &item.sources {
                driven.push(
                    (0..freqs.len())
                        .map(|bin| source[bin] * item.blocks_h[bin] * (request.voltage_v * sign))
                        .collect(),
                );
            }
        }
        let mut failure: Option<String> = None;
        let mut image_sound: Vec<Vec<Complex64>> = Vec::new();
        for image in &images {
            match radiation.transfer(&scene, &all_patches, &freqs, image.point_m, &mut *progress) {
                Ok(transfer) => {
                    add_notes(&mut fidelity, &transfer.notes);
                    image_sound.push(
                        (0..freqs.len())
                            .map(|bin| {
                                driven
                                    .iter()
                                    .zip(&transfer.per_source)
                                    .map(|(velocity, row)| velocity[bin] * row[bin])
                                    .sum()
                            })
                            .collect(),
                    );
                }
                Err(err) => {
                    failure = Some(err);
                    break;
                }
            }
        }
        let mut power_w: Vec<f64> = Vec::new();
        if failure.is_none() && wants_power {
            match radiation.sound_power(&scene, &all_patches, &driven, &freqs, &mut *progress) {
                Ok(power) => {
                    add_notes(&mut fidelity, &power.notes);
                    power_w = power.power_w;
                }
                Err(err) => failure = Some(err),
            }
        }
        if let Some(err) = failure {
            issues.push(Issue::warning(
                "room_failed",
                format!(
                    "The sound around the cabinet could not be computed: {err} These results \
                     are the direct sound only."
                ),
                "speaker.room",
            ));
            fidelity.room = "free".to_string();
        } else {
            if !power_w.is_empty() {
                let distance = design.speaker.listening.distance_m;
                sound_power_db = Some(
                    power_w
                        .iter()
                        .map(|watts| 10.0 * (watts.max(1e-30) / 1e-12).log10())
                        .collect(),
                );
                // On-axis intensity against the mean over all directions.
                directivity_index_db = Some(
                    power_w
                        .iter()
                        .zip(&summed_pressure)
                        .map(|(watts, pressure)| {
                            let mean = watts.max(1e-30) * RHO0 * C0
                                / (4.0 * std::f64::consts::PI * distance * distance);
                            10.0 * (pressure.norm_sqr().max(1e-30) / mean).log10()
                        })
                        .collect(),
                );
            }
            if let Some(found) = room.in_room(&RoomSound {
                freqs_hz: &freqs,
                direct: &summed_pressure,
                images: &image_sound,
                power_w: &power_w,
            }) {
                let level = |pascal_squared: f64| {
                    10.0 * (pascal_squared.max(1e-30) / (P_REF * P_REF)).log10()
                };
                let mut report = InRoomReport {
                    spl_db: Vec::with_capacity(freqs.len()),
                    early_spl_db: Vec::with_capacity(freqs.len()),
                    reflection_spl_db: Vec::with_capacity(freqs.len()),
                    diffuse_spl_db: Vec::with_capacity(freqs.len()),
                };
                for (bin, direct) in summed_pressure.iter().enumerate() {
                    let early = (*direct + found.reflection[bin]).norm_sqr();
                    let diffuse = found.diffuse_pa[bin] * found.diffuse_pa[bin];
                    report.spl_db.push(level(early + diffuse));
                    report.early_spl_db.push(level(early));
                    report
                        .reflection_spl_db
                        .push(level(found.reflection[bin].norm_sqr()));
                    report.diffuse_spl_db.push(level(diffuse));
                }
                in_room = Some(report);
                add_notes(&mut fidelity, &room.notes());
                if any_measured {
                    add_notes(
                        &mut fidelity,
                        &[
                            "The room's share of the sound uses the piston model, without the \
                           drivers' published curves."
                                .to_string(),
                        ],
                    );
                }
            }
        }
    }

    // A design with a room is scored on its in-room response (README section 11.1).
    let scored_spl_db: &[f64] = in_room
        .as_ref()
        .map_or(summed_spl_db.as_slice(), |report| report.spl_db.as_slice());
    // The system plays louder until the first radiator reaches a limit; each radiator gets the
    // input through its own crossover and EQ, so a woofer far above its crossover never limits.
    let max_spl_sum: Vec<f64> = if radiators.is_empty() {
        vec![f64::MAX; freqs.len()]
    } else {
        scored_spl_db
            .iter()
            .zip(&input_limit)
            .map(|(level, limit)| {
                level - 20.0 * request.voltage_v.max(1e-12).log10()
                    + 20.0 * limit.max(1e-12).log10()
            })
            .collect()
    };
    let (f6_hz, axis_flatness_db, max_spl_metric_db) = metrics(&freqs, scored_spl_db, &max_spl_sum);

    if request.losses && !design.speaker.enclosure.ducts.is_empty() {
        fidelity.ignored.push(
            "Air paths lose sound at their walls as a smooth round duct of the same area would; \
             a slot, a rough printed wall, or a bend loses more."
                .into(),
        );
    }
    // The listening window (README section 11.1): the direct sound averaged, as energy, over
    // straight ahead, 10, 20, and 30 degrees to either side, and 10 degrees up and down.
    let mut listening_window_db: Option<Vec<f64>> = None;
    let mut window_flatness_db: Option<f64> = None;
    if request.listening_window && !prepared.is_empty() {
        if request.radiation == "exterior" {
            fidelity.ignored.push(
                "The listening window is not computed with the outside in 3D (one 3D run per \
                 direction); evenness is the level straight ahead only."
                    .into(),
            );
        } else {
            let distance = design.speaker.listening.distance_m;
            let mut energy = vec![0.0; freqs.len()];
            let mut directions = 0.0;
            let mut failed = false;
            for (side_deg, up_deg) in [
                (0.0f64, 0.0f64),
                (10.0, 0.0),
                (-10.0, 0.0),
                (20.0, 0.0),
                (-20.0, 0.0),
                (30.0, 0.0),
                (-30.0, 0.0),
                (0.0, 10.0),
                (0.0, -10.0),
            ] {
                let (side, up) = (side_deg.to_radians(), up_deg.to_radians());
                let point = [
                    reference[0] + distance * side.sin() * up.cos(),
                    reference[1] - distance * side.cos() * up.cos(),
                    reference[2] + distance * up.sin(),
                ];
                let pressure: Vec<Complex64> = if side_deg == 0.0 && up_deg == 0.0 {
                    summed_pressure.clone()
                } else {
                    match radiation.transfer(&scene, &all_patches, &freqs, point, &mut *progress) {
                        Ok(transfer) => {
                            let mut total = vec![Complex64::new(0.0, 0.0); freqs.len()];
                            let mut row = 0;
                            for (place, item) in prepared.iter().enumerate() {
                                let sign = if flipped == Some(place) { -1.0 } else { 1.0 };
                                for source in &item.sources {
                                    for bin in 0..freqs.len() {
                                        total[bin] += source[bin]
                                            * item.blocks_h[bin]
                                            * (request.voltage_v * sign)
                                            * transfer.per_source[row][bin];
                                    }
                                    row += 1;
                                }
                            }
                            total
                        }
                        Err(_) => {
                            failed = true;
                            break;
                        }
                    }
                };
                for (sum, value) in energy.iter_mut().zip(&pressure) {
                    *sum += value.norm_sqr();
                }
                directions += 1.0;
            }
            if !failed {
                let window: Vec<f64> = energy
                    .iter()
                    .map(|sum| 10.0 * (sum / directions / (P_REF * P_REF)).max(1e-30).log10())
                    .collect();
                let (_, flat, _) = metrics(&freqs, &window, &max_spl_sum);
                window_flatness_db = Some(flat);
                listening_window_db = Some(window);
            }
        }
    }
    let flatness_db = window_flatness_db.map_or(axis_flatness_db, |w| w.max(axis_flatness_db));

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
        cost: design_cost(design),
        elapsed_ms: elapsed_ms(started),
        f6_hz,
        flatness_db,
        max_spl_metric_db,
        fdtd: fdtd_report,
        sound_power_db,
        listening_window_db,
        axis_flatness_db,
        window_flatness_db,
        directivity_index_db,
        in_room,
    }
}

/// README 5.5: of a wall's loss to sound grazing it, the share that sound arriving from all
/// directions alike loses on average. Friction is 1/1.4737 of the grazing loss and counts
/// with sin^2 of the angle (mean 2/3); heat flow is the rest and counts in full.
const WALL_DIFFUSE: f64 = (2.0 / 3.0 + 0.4737) / 1.4737;

struct Full3d {
    report: FdtdReport,
    /// `Z_ab` per frequency, where the grid resolves it.
    z: Vec<Option<Complex64>>,
    /// `H_k` per port and frequency (zero where the grid does not resolve it).
    h: Vec<Vec<Complex64>>,
    notes: Vec<String>,
}

/// Runs the full 3D model on the cabinet as the builder makes it (README section 5.5).
/// The size of `Z_ab` that matters to the first driver that loads the cabinet, at each
/// frequency: its own mechanical side seen from the air. A cone's volume velocity is
/// `A / (B + C * Z_ab)`, so two evaluations (with no air load and with `z_ab`) give
/// `B / C = Z_ab / (U(0) / U(z_ab) - 1)` without opening the radiator. Empty when no driver
/// loads the cabinet.
fn driver_impedance_scale(
    design: &DesignFile,
    registry: &Registry,
    freqs: &[f64],
    z_ab: &[Complex64],
) -> Vec<f64> {
    for part in &design.speaker.radiators {
        let Ok(radiator) = registry.create_radiator(&part.kind, &part.fields) else {
            continue;
        };
        if !radiator.loads_box() || !driver_loads_box(part) {
            continue;
        }
        let free = radiator.evaluate(freqs, &vec![Complex64::new(0.0, 0.0); freqs.len()]);
        let loaded = radiator.evaluate(freqs, z_ab);
        return free
            .volume_velocity
            .iter()
            .zip(&loaded.volume_velocity)
            .zip(z_ab)
            .map(|((u0, u1), z)| {
                let ratio = u0 / u1 - Complex64::new(1.0, 0.0);
                let scale = (z / ratio).norm();
                if scale.is_finite() {
                    scale
                } else {
                    1e30
                }
            })
            .collect();
    }
    Vec::new()
}

/// The 3D grid of the cabinet as the builder makes it, driven by the first cone that opens
/// into its air.
pub(crate) fn cabinet_grid_of(
    design: &DesignFile,
    outer: &dyn OuterShape,
    dx: f64,
) -> Result<voxels::CabinetGrid, String> {
    let part = design
        .speaker
        .radiators
        .iter()
        .find(|part| part.kind == "cone" && driver_loads_box(part))
        .ok_or("no driver opens into the cabinet's air.")?;
    let mount_id = part
        .fields
        .get("mount")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mount = design
        .speaker
        .enclosure
        .mounts
        .iter()
        .find(|mount| mount.id == mount_id)
        .ok_or("the driver's mount is missing.")?;
    let pose = outer.mount_pose(&mount.surface, mount.at_m)?;
    let wall = outer.panel_thickness_m(&mount.surface)?;
    let piston = piston_radius_m(part);
    let body_radius = mount_cutout(design, mount_id)
        .map(|d| d / 2.0)
        .unwrap_or(piston);
    let (displacement, _, _) = cone_displacement_m3(&part.fields);
    let body_length = if body_radius > 0.0 {
        displacement / (std::f64::consts::PI * body_radius * body_radius)
    } else {
        0.0
    };
    let chamber_list = chambers::for_simulation(design, outer)?;
    let mut bores = Vec::new();
    for runs in ports::bores(design) {
        bores.extend(runs?);
    }
    // A moulded duct's mouth: two cells deep, enough to carry the opening's faces.
    bores.extend(ports::moulded_mouths(outer, 2.0 * dx));
    let placed = solids::solids(design)?;
    voxels::cabinet_grid(
        outer,
        &chamber_list,
        &bores,
        &placed,
        &voxels::DriverPlacement {
            pose,
            wall_m: wall,
            piston_radius_m: piston,
            body_radius_m: body_radius,
            body_length_m: body_length,
        },
        dx,
    )
}

#[allow(clippy::too_many_arguments)]
fn full_3d_enclosure(
    design: &DesignFile,
    outer: &dyn OuterShape,
    freqs: &[f64],
    net_volume_m3: f64,
    // Detailed (or quick) model Z_ab: self-check compares the 3D result to this, not to a
    // one-chamber vented_sample of the total volume (that broke on the LCM).
    z_ab_reference: &[Complex64],
    dx: f64,
    full_decay: bool,
    losses: bool,
    z_scale: &[f64],
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> Result<Full3d, String> {
    if !(0.002..=0.05).contains(&dx) {
        return Err(format!(
            "a cell of {:.1} mm is outside the 2 to 50 mm this solver takes.",
            dx * 1000.0
        ));
    }
    let built = cabinet_grid_of(design, outer, dx)?;
    let damping = built.damping.clone();
    let mut grid = built.grid;
    grid.true_volume_m3 = net_volume_m3;
    let valid = C0 / (10.0 * dx);
    let resolved: Vec<f64> = freqs.iter().copied().filter(|f| *f <= valid).collect();
    let dt = grid.time_step_s();
    // The air's loss at the cabinet's walls (README 5.5), as one average: wall area over
    // air volume, both counted on the grid.
    let mut loss_notes: Vec<String> = Vec::new();
    let loss_per_s: Vec<f64> = if losses {
        let [nx, ny, nz] = grid.n;
        let stride = [ny * nz, nz, 1usize];
        let (mut faces, mut cells) = (0usize, 0usize);
        for (cell, inside) in built.chamber_air.iter().enumerate() {
            if !inside {
                continue;
            }
            cells += 1;
            let c = [cell / (ny * nz), (cell / nz) % ny, cell % nz];
            for axis in 0..3 {
                let n = [nx, ny, nz][axis];
                if c[axis] == 0 || !grid.air[cell - stride[axis]] {
                    faces += 1;
                }
                if c[axis] + 1 >= n || !grid.air[cell + stride[axis]] {
                    faces += 1;
                }
            }
        }
        let per_m = faces as f64 / (cells.max(1) as f64 * dx);
        loss_notes.push(format!(
            "The air's loss at the walls is one average for the whole cabinet ({:.0} m^2 of \
             wall per m^3 of air, counted on the grid; sound taken to travel in all directions \
             alike). One resonance's own loss can differ from it by about half, and curved or \
             slanted walls count a little large.",
            per_m
        ));
        resolved
            .iter()
            .map(|f| C0 * WALL_DIFFUSE * seatlock_acoustics::WALL_LOSS * f.sqrt() * per_m)
            .collect()
    } else {
        Vec::new()
    };
    let options = seatlock_fdtd::grid::Options {
        max_steps: (1.0 / dt).ceil() as usize,
        frequencies_hz: resolved.clone(),
        progress_every: 250,
        extrapolate: !full_decay,
        z_scale: freqs
            .iter()
            .zip(z_scale)
            .filter(|(f, _)| **f <= valid)
            .map(|(_, z)| *z)
            .collect(),
        relaxed: Vec::new(),
        loss_per_s,
        damping,
    };
    let response = seatlock_fdtd::grid::run(&grid, &options, progress);
    if response.canceled {
        return Err("it was canceled.".into());
    }
    let mut z = vec![None; freqs.len()];
    let mut h = vec![vec![Complex64::new(0.0, 0.0); freqs.len()]; grid.openings.len()];
    let mut z_quick = Vec::new();
    let mut worst: Option<f64> = None;
    let check_below = C0 / (50.0 * built.longest_m.max(dx));
    let mut cursor = 0usize;
    for (bin, freq) in freqs.iter().enumerate() {
        if *freq > valid {
            continue;
        }
        let value = Complex64::new(response.z_real[cursor], response.z_imag[cursor]);
        z[bin] = Some(value);
        for (port, values) in h.iter_mut().enumerate() {
            values[bin] =
                Complex64::new(response.h_real[port][cursor], response.h_imag[port][cursor]);
        }
        cursor += 1;
        let quick = z_ab_reference
            .get(bin)
            .copied()
            .unwrap_or(Complex64::new(0.0, 0.0));
        z_quick.push(quick);
        if *freq < check_below && quick.norm() > 0.0 {
            let error = (value - quick).norm() / quick.norm();
            worst = Some(worst.map_or(error, |w: f64| w.max(error)));
        }
    }
    let report = FdtdReport {
        dx_m: dx,
        cells: grid.n[0] * grid.n[1] * grid.n[2],
        steps: response.steps,
        decayed: response.decayed,
        elapsed_ms: response.elapsed_ms,
        valid_up_to_hz: valid,
        grid_volume_m3: built.inside_volume_m3,
        net_volume_m3,
        self_check_error: worst,
        frequencies_hz: resolved,
        z_ab: ComplexCurve {
            re: response.z_real.clone(),
            im: response.z_imag.clone(),
        },
        z_ab_quick: curve_from(&z_quick),
        port_ids: built.port_ids,
    };
    Ok(Full3d {
        report,
        z,
        h,
        notes: built.notes.into_iter().chain(loss_notes).collect(),
    })
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

/// A driver whose independent measurement found a cone break-up peak (README 9.1) is asked to
/// stay at least an octave below it: a fourth-order low-pass is 24 dB down there, which
/// leaves a typical peak well under the rest. The cone model itself has no break-up.
fn breakup_warnings(design: &DesignFile) -> Vec<Issue> {
    let mut out = Vec::new();
    for (index, radiator) in design.speaker.radiators.iter().enumerate() {
        let Some(id) = radiator.fields.get("driver_id").and_then(Value::as_str) else {
            continue;
        };
        let Some((f, peak)) = seatlock_core::catalog::driver(id)
            .as_ref()
            .and_then(seatlock_acoustics::breakup)
        else {
            continue;
        };
        let low_pass = design.dsp.per_radiator.get(index).and_then(|chain| {
            chain
                .blocks
                .iter()
                .filter(|block| {
                    block.kind == "crossover"
                        && block.fields.get("band").and_then(Value::as_str) == Some("low")
                })
                .filter_map(|block| block.fields.get("f_hz").and_then(Value::as_f64))
                .reduce(f64::min)
        });
        let message = match low_pass {
            Some(cut) if cut <= f / 2.0 => continue,
            Some(cut) => format!(
                "{id}: an independent test found a {peak:.0} dB cone break-up peak at {:.1} kHz, and its low-pass at {:.1} kHz leaves it in play. The cone model does not show it; move the crossover to {:.1} kHz or lower.",
                f / 1000.0,
                cut / 1000.0,
                f / 2000.0
            ),
            None => format!(
                "{id}: an independent test found a {peak:.0} dB cone break-up peak at {:.1} kHz, and nothing filters it out. The cone model does not show it, so the treble here reads smoother than it will sound.",
                f / 1000.0
            ),
        };
        out.push(Issue::warning(
            "measured_breakup",
            message,
            format!("dsp.per_radiator[{index}]"),
        ));
    }
    out
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

/// The plain-language message for a shape that cannot answer part of its contract.
fn shape_message(kind: &str, err: &str) -> String {
    if err == seatlock_core::registry::SHAPE_NOT_FINISHED {
        format!("The {kind} cabinet shape is not finished yet, so this design is left out of the results.")
    } else {
        err.to_string()
    }
}

struct PlacedSource {
    position: [f64; 3],
    outward: [f64; 3],
    radius_m: f64,
}

/// True when `point`, on a flat surface with the normal `outward`, is inside the surface's
/// outline `corners` (crossing count in the surface's own plane).
fn inside_outline(point: [f64; 3], corners: &[[f64; 3]], outward: [f64; 3]) -> bool {
    if corners.len() < 3 {
        return true;
    }
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut along = sub(corners[1], corners[0]);
    let length = dot(along, along).sqrt().max(1e-12);
    along = [along[0] / length, along[1] / length, along[2] / length];
    let across = [
        outward[1] * along[2] - outward[2] * along[1],
        outward[2] * along[0] - outward[0] * along[2],
        outward[0] * along[1] - outward[1] * along[0],
    ];
    let flat = |p: [f64; 3]| {
        let offset = sub(p, corners[0]);
        (dot(offset, along), dot(offset, across))
    };
    let (x, y) = flat(point);
    let mut inside = false;
    let mut previous = flat(corners[corners.len() - 1]);
    for corner in corners {
        let current = flat(*corner);
        if (current.1 > y) != (previous.1 > y) {
            let crossing =
                current.0 + (y - current.1) * (previous.0 - current.0) / (previous.1 - current.1);
            if x < crossing {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn placed_sources(
    design: &DesignFile,
    shape: &dyn OuterShape,
    part: &KindPart,
    count: usize,
) -> Vec<PlacedSource> {
    let mut placed = Vec::new();
    let Some(pose) = mount_pose(design, shape, part) else {
        return placed;
    };
    placed.push(PlacedSource {
        position: pose.position_m,
        outward: pose.outward,
        radius_m: piston_radius_m(part),
    });
    if count > 1 {
        for port in port_poses(design, shape) {
            placed.push(port);
            if placed.len() == count {
                break;
            }
        }
    }
    placed
}

/// The radiator's mount position and direction, from the shape. `None` when the radiator
/// names no mount, or the shape cannot place it.
fn mount_pose(
    design: &DesignFile,
    shape: &dyn OuterShape,
    part: &KindPart,
) -> Option<seatlock_core::registry::MountPose> {
    let mount_id = part.fields.get("mount").and_then(Value::as_str)?;
    let mount = design
        .speaker
        .enclosure
        .mounts
        .iter()
        .find(|mount| mount.id == mount_id)?;
    shape.mount_pose(&mount.surface, mount.at_m).ok()
}

fn port_poses(design: &DesignFile, shape: &dyn OuterShape) -> Vec<PlacedSource> {
    let depth = shape.outer_size_m().map(|size| size[1]).unwrap_or(0.0);
    let mut poses = Vec::new();
    for duct in &design.speaker.enclosure.ducts {
        let from = duct.get("from").and_then(Value::as_str).unwrap_or("");
        let to = duct.get("to").and_then(Value::as_str).unwrap_or("");
        let leaves = (from == "outside") != (to == "outside");
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
        let points = duct.get("path_m").and_then(Value::as_array);
        let (start, end) = match points {
            Some(points) if points.len() >= 2 && to == "outside" => (
                point3(&points[points.len() - 2]),
                point3(points.last().unwrap()),
            ),
            Some(points) if points.len() >= 2 => (point3(&points[1]), point3(&points[0])),
            _ => continue,
        };
        let direction = [end.0 - start.0, end.1 - start.1, end.2 - start.2];
        let length = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt()
            .max(1e-9);
        let outward = [
            direction[0] / length,
            direction[1] / length,
            direction[2] / length,
        ];
        let (position, face) = if outward[1] < -0.5 {
            ([end.0, 0.0, end.2], [0.0, -1.0, 0.0])
        } else if outward[1] > 0.5 {
            ([end.0, depth, end.2], [0.0, 1.0, 0.0])
        } else {
            ([end.0, end.1, end.2], outward)
        };
        poses.push(PlacedSource {
            position,
            outward: face,
            radius_m: diameter / 2.0,
        });
    }
    // Ducts moulded into an imported model follow the design's, in the same order as the
    // air network's ports.
    for opening in shape.moulded_openings() {
        poses.push(PlacedSource {
            position: opening.mouth.position_m,
            outward: opening.mouth.outward,
            radius_m: (opening.mouth_area_m2() / std::f64::consts::PI).sqrt(),
        });
    }
    poses
}

fn piston_radius_m(part: &KindPart) -> f64 {
    let Some(id) = part.fields.get("driver_id").and_then(Value::as_str) else {
        return 0.0;
    };
    let sd = catalog::driver(id)
        .and_then(|record| record.pointer("/published/sd_m2").cloned())
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    (sd / std::f64::consts::PI).sqrt()
}

fn measured_curve(part: &KindPart) -> Vec<(f64, f64)> {
    let Some(id) = part.fields.get("driver_id").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(record) = catalog::driver(id) else {
        return Vec::new();
    };
    let Some(path) = record.get("response_file").and_then(Value::as_str) else {
        return Vec::new();
    };
    let Some(text) = catalog::embedded_file(path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    value
        .get("points")
        .and_then(Value::as_array)
        .map(|points| {
            points
                .iter()
                .filter_map(|point| {
                    let hz = point.get("hz")?.as_f64()?;
                    let db = point.get("db")?.as_f64()?;
                    Some((hz, db))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn geometric_delay_s(design: &DesignFile, index: usize) -> f64 {
    let width = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("width_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.2);
    let height = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("height_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.3);
    let depth = design
        .speaker
        .enclosure
        .outer
        .fields
        .get("depth_m")
        .and_then(Value::as_f64)
        .unwrap_or(0.2);
    let listener = [
        width / 2.0,
        -design.speaker.listening.distance_m,
        height / 2.0,
    ];
    let distances: Vec<f64> = design
        .speaker
        .radiators
        .iter()
        .map(|radiator| {
            let mount_id = radiator
                .fields
                .get("mount")
                .and_then(Value::as_str)
                .unwrap_or("");
            let mount = design
                .speaker
                .enclosure
                .mounts
                .iter()
                .find(|mount| mount.id == mount_id);
            let (u, v, surface) = mount
                .map(|mount| (mount.at_m[0], mount.at_m[1], mount.surface.as_str()))
                .unwrap_or((width / 2.0, height / 2.0, "front"));
            let position = match surface {
                "back" => [u, depth, v],
                _ => [u, 0.0, v],
            };
            let dx = position[0] - listener[0];
            let dy = position[1] - listener[1];
            let dz = position[2] - listener[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .collect();
    let nearest = distances.iter().copied().fold(f64::MAX, f64::min);
    distances
        .get(index)
        .map(|distance| ((*distance - nearest) / C0).max(0.0))
        .unwrap_or(0.0)
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
        // Round or slot, straight or tapered: the same section as the air network reads.
        let Ok(section) = chambers::Section::of(duct) else {
            continue;
        };
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
        // The air mass `rho * L_eff / S` with a taper's equivalent area; the inside end flush
        // with the chamber wall, the outside end free.
        let area = if length > 0.0 {
            length / section.inertance_length_over_area(length)
        } else {
            section.area(0.0, 0.0)
        };
        ports.push(PortSpec {
            area_m2: area,
            length_eff_m: length
                + seatlock_acoustics::end_correction_m(section.end_radius(from == "outside"), true)
                + seatlock_acoustics::end_correction_m(
                    section.end_radius(from != "outside"),
                    false,
                ),
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
    pub warnings: Vec<String>,
    pub can_export_print: bool,
    pub cut_list: Vec<String>,
    pub print_note: String,
    pub user_wall: bool,
    /// The printed parts as a CSG tree for manifold-3d on the page (README section 6.3).
    pub print_tree: Value,
    /// How the print is cut into parts that fit the printer, with alignment pins (6.4).
    pub print_split: split::PrintSplit,
    pub cost: CostReport,
}

pub fn geometry_report(design: &DesignFile) -> Result<GeometryReport, String> {
    geometry_report_with(design, true)
}

/// [`geometry_report`]; without `voxels` the 5 mm voxel air volume (CHK-GEO-1's third view,
/// most of the time) is skipped and reported as 0. Cost does not use it.
pub fn geometry_report_with(design: &DesignFile, voxels: bool) -> Result<GeometryReport, String> {
    let outer = &design.speaker.enclosure.outer;
    let registry = builtin();
    let shape = registry.create_outer(&outer.kind, &outer.fields)?;
    let kind = outer.kind.as_str();
    let [width_m, depth_m, height_m] = shape
        .outer_size_m()
        .map_err(|err| shape_message(kind, &err))?;
    let mut warnings = shape.notes();
    let user_wall = warnings.iter().any(|line| line.contains("user's own"));
    let mut mounts = Vec::new();
    let mut cut_areas = Vec::new();
    let mut holes = Vec::new();
    let mut can_export = true;
    let mut features = Vec::new();
    let mut feature_volume = 0.0;
    for mount in &design.speaker.enclosure.mounts {
        if mount.surface.is_empty() {
            // An imported cabinet before its sound sources are placed: show it, cut nothing.
            warnings.push(format!(
                "Mount {}: not placed yet. Choose Place on model and click where it goes.",
                mount.id
            ));
            can_export = false;
            continue;
        }
        let pose = shape
            .mount_pose(&mount.surface, mount.at_m)
            .map_err(|err| format!("Mount {}: {}", mount.id, shape_message(kind, &err)))?;
        let cutout = match mount_cutout(design, &mount.id) {
            Some(value) if value > 0.0 => value,
            _ if mount_holds_no_driver(design, &mount.id) => {
                // A sound source that is not a driver (a panel, or a kind this build does not
                // know) sits on the wall. The lab does not know a hole for it, so it cuts none.
                warnings.push(format!(
                    "The sound source on mount {} is not a driver, so nothing is cut for it: the wall behind it stays whole, and whatever holds it (a panel's frame and the shallow sealed space behind it) is not in the print file or the cost.",
                    mount.id
                ));
                0.0
            }
            _ => {
                warnings.push(
                    "mounting size not published, so this mount is not cut into the print file."
                        .into(),
                );
                can_export = false;
                0.0
            }
        };
        if cutout > 0.0 {
            cut_areas.push((
                mount.surface.clone(),
                std::f64::consts::PI * (cutout * 0.5).powi(2),
            ));
            holes.push((pose, cutout));
            let thickness = shape.panel_thickness_m(&mount.surface).unwrap_or(0.0);
            match mount_features(design, mount, &pose, cutout, thickness) {
                Ok((parts, volume, note)) => {
                    features.extend(parts);
                    feature_volume += volume;
                    if let Some(note) = note {
                        warnings.push(format!("Mount {}: {note}", mount.id));
                    }
                }
                Err(problem) => {
                    warnings.push(format!("Mount {}: {problem}", mount.id));
                    can_export = false;
                }
            }
        }
        mounts.push(MountView {
            surface: mount.surface.clone(),
            at_m: mount.at_m,
            position_m: pose.position_m,
            outward: pose.outward,
            cutout_diameter_m: cutout,
        });
    }
    let solid = shape
        .print_solid_m3(&cut_areas)
        .map_err(|err| shape_message(kind, &err))?;
    let solid = if solid > 0.0 {
        (solid - feature_volume).max(0.0)
    } else {
        solid
    };
    let mut print_tree = shape
        .print_tree(&holes)
        .map_err(|err| shape_message(kind, &err))?;
    let mut solid = solid;
    let mut shells = Vec::new();
    let mut bores = Vec::new();
    let chamber_list = match chambers::chambers(design) {
        Ok(list) => list,
        Err(problem) => {
            warnings.push(problem);
            can_export = false;
            Vec::new()
        }
    };
    if !chamber_list.is_empty() {
        // Partition walls: the inside air that no chamber claims is printed solid.
        let air = shape.air_tree();
        let bad = chamber_list.iter().find(|chamber| {
            (0..8).any(|corner| {
                let inset = 1e-6;
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
                !shape.contains_air(p).unwrap_or(false)
            })
        });
        match (air, bad) {
            (_, Some(chamber)) => {
                warnings.push(format!(
                    "Chamber {} reaches outside the cabinet's inside air; the print file is refused.",
                    chamber.id
                ));
                can_export = false;
            }
            (Err(_), None) => {
                warnings.push(
                    "Partitions between chambers are not made for this kind of cabinet yet; the \
                     print file is refused."
                        .into(),
                );
                can_export = false;
            }
            (Ok(air), None) => {
                let mut parts = vec![air];
                parts.extend(
                    chamber_list
                        .iter()
                        .map(|chamber| seatlock_geom::csg_box(chamber.min_m, chamber.max_m)),
                );
                let claimed: f64 = chamber_list.iter().map(chambers::Chamber::box_m3).sum();
                if solid > 0.0 {
                    solid += shape.interior_volume_m3()? - claimed;
                }
                print_tree = seatlock_geom::csg_union(vec![
                    print_tree,
                    seatlock_geom::csg_difference(parts),
                ]);
            }
        }
        // Each driver must open into the chamber it names, not into a partition.
        for (view, mount) in mounts.iter().zip(&design.speaker.enclosure.mounts) {
            let thickness = shape.panel_thickness_m(&mount.surface).unwrap_or(0.0) + 0.002;
            let behind = [
                view.position_m[0] - view.outward[0] * thickness,
                view.position_m[1] - view.outward[1] * thickness,
                view.position_m[2] - view.outward[2] * thickness,
            ];
            let opens = chamber_list
                .iter()
                .find(|chamber| chamber.id == mount.rear_into)
                .is_some_and(|chamber| chamber.contains(behind));
            if !opens {
                warnings.push(format!(
                    "Mount {} does not open into chamber {}; the print file is refused.",
                    mount.id, mount.rear_into
                ));
                can_export = false;
            }
        }
    }
    for port in ports::printed_ports(design, shape.as_ref(), &chamber_list) {
        let port = match port {
            Ok(port) => port,
            Err(problem) => {
                warnings.push(problem);
                can_export = false;
                continue;
            }
        };
        let clash = mounts
            .iter()
            .zip(&design.speaker.enclosure.mounts)
            .find(|(view, mount)| {
                let d = [
                    view.position_m[0] - port.opening_m[0],
                    view.position_m[1] - port.opening_m[1],
                    view.position_m[2] - port.opening_m[2],
                ];
                let flange = mount_driver(design, &mount.id)
                    .and_then(|driver| {
                        driver
                            .pointer("/mounting/outer_diameter_m")
                            .and_then(Value::as_f64)
                    })
                    .unwrap_or(view.cutout_diameter_m);
                view.surface == port.surface
                    && (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                        < flange / 2.0 + port.half_width_m
            });
        if let Some((_, mount)) = clash {
            warnings.push(format!(
                "Port {}: its opening overlaps the {} mount; the print file is refused.",
                port.id, mount.id
            ));
            can_export = false;
            continue;
        }
        if solid > 0.0 {
            solid += port.solid_m3;
        }
        shells.push(port.shell);
        bores.push(port.bore);
    }
    // Plastic placed in the air (`voxels` and `brace` solids): the part of it that lies in
    // the air.
    let placed = match solids::solids(design) {
        Ok(found) => found,
        Err(problem) => {
            warnings.push(problem);
            can_export = false;
            solids::Placed::default()
        }
    };
    for (id, kind) in &placed.unknown {
        warnings.push(format!(
            "Solid {id} ({kind}) is of a kind this build does not make yet: it is left out of \
             the results, the print file, and the cost."
        ));
    }
    let in_air_at = |p: [f64; 3]| -> bool {
        shape.contains_air(p).unwrap_or(false)
            && (chamber_list.is_empty() || chamber_list.iter().any(|c| c.contains(p)))
    };
    let placed_m3 = placed.plastic_m3(&in_air_at);
    if placed_m3 > 0.0 {
        let air = if chamber_list.is_empty() {
            shape.air_tree()
        } else {
            Ok(seatlock_geom::csg_union(
                chamber_list
                    .iter()
                    .map(|c| seatlock_geom::csg_box(c.min_m, c.max_m))
                    .collect(),
            ))
        };
        match air {
            Ok(air) if solid > 0.0 => {
                let blocks = || {
                    seatlock_geom::csg_union(
                        placed
                            .print_boxes()
                            .into_iter()
                            .map(|(min, max)| seatlock_geom::csg_box(min, max))
                            .collect(),
                    )
                };
                // blocks within the air = blocks - (blocks - air).
                let outside = seatlock_geom::csg_difference(vec![blocks(), air]);
                print_tree = seatlock_geom::csg_union(vec![
                    print_tree,
                    seatlock_geom::csg_difference(vec![blocks(), outside]),
                ]);
                solid += placed_m3;
            }
            Ok(_) => warnings.push(
                "Plastic placed inside a cabinet that is not printed is simulated, but it is \
                 not in a print file or the cost."
                    .into(),
            ),
            Err(_) => {
                warnings.push(
                    "Plastic placed inside is not printed for this kind of cabinet yet; the \
                     print file is refused."
                        .into(),
                );
                can_export = false;
            }
        }
    }
    if !shells.is_empty() {
        let mut body = vec![print_tree];
        body.extend(shells);
        print_tree = seatlock_geom::csg_union(body);
    }
    if !features.is_empty() || !bores.is_empty() {
        let mut parts = vec![print_tree];
        parts.extend(bores);
        parts.extend(features);
        print_tree = seatlock_geom::csg_difference(parts);
    }
    // Split into parts that fit the printer, with pin holes where there is plastic for them.
    let (_, bed) = printer_volume();
    let mut print_split = split::PrintSplit::default();
    let printed = solid > 0.0 && !shape.printed_part_sizes_m().is_empty();
    let whole_print = shape.printed_part_sizes_m().len() == 1;
    if printed && whole_print && bed.iter().all(|v| *v > 0.0) {
        let size = [width_m, depth_m, height_m];
        let mut keep_clear: Vec<([f64; 3], f64)> = mounts
            .iter()
            .zip(&design.speaker.enclosure.mounts)
            .map(|(view, mount)| {
                let flange = mount_driver(design, &mount.id)
                    .and_then(|d| {
                        d.pointer("/mounting/outer_diameter_m")
                            .and_then(Value::as_f64)
                    })
                    .unwrap_or(view.cutout_diameter_m);
                (view.position_m, flange / 2.0 + 0.003)
            })
            .collect();
        for port in ports::printed_ports(design, shape.as_ref(), &chamber_list)
            .into_iter()
            .flatten()
        {
            keep_clear.push((port.opening_m, port.half_width_m + 0.003));
        }
        let duct_lines: Vec<([f64; 3], [f64; 3], f64)> =
            ports::printed_ports(design, shape.as_ref(), &chamber_list)
                .into_iter()
                .flatten()
                .flat_map(|port| port.lines)
                .map(|(a, b, reach)| (a, b, reach + 0.003))
                .collect();
        let solid_at = |p: [f64; 3]| -> bool {
            let plastic = shape.contains_solid(p).unwrap_or(false)
                || (in_air_at(p) && placed.contains(p))
                || (!chamber_list.is_empty()
                    && shape.contains_air(p).unwrap_or(false)
                    && !chamber_list.iter().any(|c| c.contains(p)));
            if !plastic {
                return false;
            }
            let near = |a: [f64; 3], r: f64| {
                ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2) + (p[2] - a[2]).powi(2)).sqrt() < r
            };
            if keep_clear.iter().any(|(at, r)| near(*at, *r)) {
                return false;
            }
            duct_lines.iter().all(|(a, b, r)| {
                let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2])
                    / len2.max(1e-12))
                .clamp(0.0, 1.0);
                let q = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t];
                !near(q, *r)
            })
        };
        print_split = split::plan(size, bed, &solid_at);
        if !print_split.pins.is_empty() {
            let mut parts = vec![print_tree];
            parts.extend(split::hole_tree(&print_split));
            print_tree = seatlock_geom::csg_difference(parts);
            solid -= print_split.pin_holes_m3;
        }
    }
    let density = catalog::material(&design.build.material)
        .and_then(|record| record.get("density_kg_m3").and_then(Value::as_f64))
        .unwrap_or(0.0);
    let cost = cost::report(
        design,
        &CabinetParts {
            print_solid_m3: Some(solid + print_split.pins_m3),
            stock_cuts: shape.stock_cuts(),
            stock_panels: shape.stock_panels(),
            part_materials: shape.part_materials(),
            problems: Vec::new(),
        },
    );
    if !cost.problems.is_empty() || solid <= 0.0 {
        can_export = false;
    }
    let mut print_note = if print_split.parts.is_empty() {
        print_split_note(&shape.printed_part_sizes_m())
    } else {
        let (name, _) = printer_volume();
        let mut note = format!(
            "Printed in {} parts that fit the {name}, lined up by {} printed pins, 4 mm across \
             and 15 mm long.",
            print_split.parts.len(),
            print_split.pins.len()
        );
        for extra in &print_split.notes {
            note.push(' ');
            note.push_str(extra);
        }
        note
    };
    if !shape.stock_cuts().is_empty() || !shape.stock_panels().is_empty() {
        print_note.push_str(" Stock parts are cut, not printed; see the cut list.");
    }
    Ok(GeometryReport {
        width_m,
        height_m,
        depth_m,
        wall_m: shape.panel_thickness_m("front").unwrap_or(0.0),
        interior_volume_m3: if chamber_list.is_empty() {
            shape.interior_volume_m3()? - placed_m3
        } else {
            chamber_list.iter().map(chambers::Chamber::volume_m3).sum()
        },
        solid_volume_m3: solid,
        voxel_air_volume_m3: if voxels {
            seatlock_geom::voxel_air_volume_where(shape.as_ref(), 0.005, &|p| {
                (chamber_list.is_empty() || chamber_list.iter().any(|c| c.contains(p)))
                    && !placed.contains(p)
            })?
        } else {
            0.0
        },
        print_mass_kg: (solid + print_split.pins_m3) * density * design.build.infill_fraction,
        cost_note: catalog::cost_note(design),
        mounts,
        warnings,
        can_export_print: can_export,
        cut_list: cost.cut_list.clone(),
        print_note,
        user_wall,
        print_tree,
        print_split,
        cost,
    })
}

/// The cost of a design, whatever its shape. A shape without a geometry report still gets
/// its drivers and electronics priced, and says why the cabinet is missing.
pub fn design_cost(design: &DesignFile) -> CostReport {
    match geometry_report_with(design, false) {
        Ok(report) => report.cost,
        Err(err) => cost::report(
            design,
            &CabinetParts {
                print_solid_m3: None,
                stock_cuts: Vec::new(),
                stock_panels: Vec::new(),
                part_materials: Vec::new(),
                problems: vec![err],
            },
        ),
    }
}

/// The owner's printer from the catalog: name and main-nozzle build volume in m.
fn printer_volume() -> (String, [f64; 3]) {
    let record = catalog::printer(None);
    let name = record
        .as_ref()
        .map(|r| {
            format!(
                "{} {}",
                r.get("maker").and_then(Value::as_str).unwrap_or(""),
                r.get("model").and_then(Value::as_str).unwrap_or("")
            )
            .trim()
            .to_string()
        })
        .unwrap_or_else(|| "printer".into());
    let volume = record
        .as_ref()
        .and_then(|r| r.get("build_volume_m").and_then(Value::as_array))
        .and_then(|values| {
            let v: Vec<f64> = values.iter().filter_map(Value::as_f64).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    (name, volume)
}

fn print_split_note(parts: &[[f64; 3]]) -> String {
    if parts.is_empty() {
        return "Nothing is printed.".to_string();
    }
    let (name, [bx, by, bz]) = printer_volume();
    if bx <= 0.0 || by <= 0.0 || bz <= 0.0 {
        return "No printer is in the catalog, so the print is not split.".to_string();
    }
    let size = format!(
        "{:.0} x {:.0} x {:.0} mm",
        bx * 1000.0,
        by * 1000.0,
        bz * 1000.0
    );
    let pieces = |size: f64, limit: f64| (size / limit).ceil().max(1.0) as i32;
    let count: i32 = parts
        .iter()
        .map(|[x, y, z]| pieces(*x, bx) * pieces(*y, by) * pieces(*z, bz))
        .sum();
    if count as usize <= parts.len() {
        format!("Every printed part fits the {name} ({size}).")
    } else {
        format!("About {count} printed pieces for the {name} ({size}).")
    }
}

/// The cut-out diameter of the driver on a mount, from its catalog record.
/// Pilot holes for the driver's screws, unless the mount gives `bolt_hole_m`. Makers print
/// the bolt circle but not a hole size, so this is the lab's own default (STATUS decisions).
pub const DEFAULT_BOLT_HOLE_M: f64 = 0.0035;

/// The driver record mounted at `mount_id`.
fn mount_driver(design: &DesignFile, mount_id: &str) -> Option<Value> {
    design
        .speaker
        .radiators
        .iter()
        .find(|radiator| radiator.fields.get("mount").and_then(Value::as_str) == Some(mount_id))
        .and_then(|radiator| radiator.fields.get("driver_id").and_then(Value::as_str))
        .and_then(catalog::driver)
}

/// The recess for the driver's flange and its bolt holes, as CSG parts to subtract, and the
/// plastic they remove beyond the cut-out. Both come from the driver's mounting record
/// (README sections 6.4 and 9.1); a size the record does not print refuses the export.
fn mount_features(
    design: &DesignFile,
    mount: &seatlock_core::design::MountRecord,
    pose: &seatlock_geom::MountPose,
    cutout_m: f64,
    thickness_m: f64,
) -> Result<(Vec<Value>, f64, Option<String>), String> {
    let driver = mount_driver(design, &mount.id).ok_or("no driver record")?;
    let size = |key: &str| {
        driver
            .pointer(&format!("/mounting/{key}"))
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0)
    };
    let pi = std::f64::consts::PI;
    let recess = mount.recess_m;
    let o = pose.outward;
    let at = |offset: [f64; 3], out: f64| {
        [
            pose.position_m[0] + offset[0] + o[0] * out,
            pose.position_m[1] + offset[1] + o[1] * out,
            pose.position_m[2] + offset[2] + o[2] * out,
        ]
    };
    let inward = [-o[0], -o[1], -o[2]];
    let extra = 0.0005;
    let mut parts = Vec::new();
    let mut volume = 0.0;
    if recess > 0.0 {
        let flange = size("outer_diameter_m").ok_or(
            "the driver's flange diameter is not published, so the recess cannot be cut and \
             the print file is refused.",
        )?;
        if recess >= thickness_m {
            return Err(format!(
                "the {:.1} mm recess is as deep as the {:.1} mm panel; the print file is refused.",
                recess * 1000.0,
                thickness_m * 1000.0
            ));
        }
        parts.push(seatlock_geom::csg_cylinder(
            at([0.0; 3], extra),
            inward,
            recess + extra,
            flange / 2.0,
        ));
        volume += pi * ((flange / 2.0).powi(2) - (cutout_m / 2.0).powi(2)) * recess;
    }
    // Without a published bolt circle the hole is still cut; the screw holes are left to the
    // builder, who marks them through the driver's own flange (README 6.4).
    let (Some(circle), Some(count)) = (
        size("bolt_circle_m"),
        size("bolt_count").map(|n| n.round() as usize),
    ) else {
        return Ok((
            parts,
            volume,
            Some(
                "the driver's bolt circle is not published, so no screw holes are in the print \
                 file: hold the driver in its hole, mark the holes through its flange, and \
                 drill them."
                    .into(),
            ),
        ));
    };
    let hole = mount
        .extra
        .get("bolt_hole_m")
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_BOLT_HOLE_M);
    if circle / 2.0 - hole / 2.0 <= cutout_m / 2.0 {
        return Err(
            "the bolt holes would break into the cut-out; the print file is refused.".into(),
        );
    }
    let (right, up) = seatlock_geom::prism::face_axes(o);
    for index in 0..count {
        let angle = 2.0 * pi * (index as f64 + 0.5) / count as f64;
        let (s, c) = angle.sin_cos();
        let r = circle / 2.0;
        let offset = [
            r * (s * right[0] + c * up[0]),
            r * (s * right[1] + c * up[1]),
            r * (s * right[2] + c * up[2]),
        ];
        parts.push(seatlock_geom::csg_cylinder(
            at(offset, extra),
            inward,
            thickness_m + 2.0 * extra,
            hole / 2.0,
        ));
        volume += pi * (hole / 2.0).powi(2) * (thickness_m - recess);
    }
    Ok((parts, volume, None))
}

/// Whether the sound source on `mount_id` names no driver: a panel, or another kind of
/// radiator that brings no cut-out of its own.
fn mount_holds_no_driver(design: &DesignFile, mount_id: &str) -> bool {
    design
        .speaker
        .radiators
        .iter()
        .find(|radiator| radiator.fields.get("mount").and_then(Value::as_str) == Some(mount_id))
        .is_some_and(|radiator| !radiator.fields.contains_key("driver_id"))
}

fn mount_cutout(design: &DesignFile, mount_id: &str) -> Option<f64> {
    design
        .speaker
        .radiators
        .iter()
        .find(|radiator| radiator.fields.get("mount").and_then(Value::as_str) == Some(mount_id))
        .and_then(|radiator| radiator.fields.get("driver_id").and_then(Value::as_str))
        .and_then(catalog::driver)
        .and_then(|driver| {
            driver
                .pointer("/mounting/cutout_diameter_m")
                .and_then(Value::as_f64)
        })
}

pub fn correction_plan_with(
    design: &DesignFile,
    registry: &Registry,
    radiation: &str,
    space: &str,
) -> Value {
    correction_plan_for(design, registry, radiation, space)
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
    correction_plan_for(design, registry, "point", "half")
}

/// Where the first radiator's own baffle edge puts the baffle step: on-axis diffraction 3 dB
/// below half space, from the outline of the surface it is mounted on. Without an outline,
/// the step formula of README section 5.6, `115 / baffle width`.
fn baffle_corner_hz(design: &DesignFile, registry: &Registry) -> f64 {
    let outer = &design.speaker.enclosure.outer;
    let Ok(shape) = registry.create_outer(&outer.kind, &outer.fields) else {
        return 0.0;
    };
    let from_outline = design.speaker.radiators.first().and_then(|part| {
        let pose = mount_pose(design, shape.as_ref(), part)?;
        let surface = shape.surface_facing(pose.outward)?;
        let corners = shape.surface_outline(&surface).ok()?;
        Some(baffle_step_hz_for_outline(
            &corners,
            pose.position_m,
            pose.outward,
        ))
    });
    from_outline.unwrap_or_else(|| 115.0 / shape.baffle_width_m().max(0.05))
}

fn correction_plan_for(
    design: &DesignFile,
    registry: &Registry,
    radiation: &str,
    space: &str,
) -> Value {
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
            enclosure: compare::ENCLOSURE.to_string(),
            radiation: radiation.to_string(),
            space: space.to_string(),
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
    let corner = baffle_corner_hz(design, registry);
    let shows_step = radiation == "baffle" || (radiation == "point" && space == "step");
    let f6_max = catalog::target(&design.target)
        .and_then(|target| target.get("f6_hz_max").and_then(Value::as_f64))
        .unwrap_or(50.0);
    let flat_max = catalog::target(&design.target)
        .and_then(|target| target.get("flatness_db_max").and_then(Value::as_f64))
        .unwrap_or(1.5);
    let (shelf_gain, shelf_note) = if shows_step {
        result
            .radiators
            .first()
            .map(|curve| {
                search_shelf(
                    curve,
                    &freqs,
                    level,
                    corner,
                    design,
                    &fixed_block_shape(design, registry, &freqs, 0),
                    protected,
                    alignment,
                    !ports.is_empty(),
                    tuning,
                    f6_max,
                    flat_max,
                )
            })
            .unwrap_or((0.0, String::new()))
    } else {
        (0.0, String::new())
    };
    let shelf_reason = if shelf_note.is_empty() {
        "Raises the bass 6 dB to make up for sound wrapping around the cabinet edge.".to_string()
    } else {
        shelf_note
    };
    let mut plans = Vec::new();
    for (index, curve) in result.radiators.iter().enumerate() {
        let fixed = fixed_block_shape(design, registry, &freqs, index);
        let (spl, max_spl) = if shelf_gain > 0.0 {
            (
                curve
                    .spl_db
                    .iter()
                    .zip(&freqs)
                    .map(|(level_db, freq)| level_db + shelf_db(shelf_gain, corner, *freq, design))
                    .collect(),
                curve
                    .max_spl_db
                    .iter()
                    .zip(&freqs)
                    .map(|(level_db, freq)| level_db - shelf_db(shelf_gain, corner, *freq, design))
                    .collect(),
            )
        } else {
            (curve.spl_db.clone(), curve.max_spl_db.clone())
        };
        let outcome = seatlock_correct::correct(
            &freqs,
            &spl,
            &max_spl,
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
        let mut filters: Vec<Value> = outcome.filters.iter().map(filter_json).collect();
        if shelf_gain > 0.0 {
            filters.insert(0, shelf_filter(corner, shelf_gain, &shelf_reason));
        }
        plans.push(serde_json::json!({
            "message": outcome.message,
            "filters": filters,
        }));
    }
    let first = plans.first().cloned().unwrap_or(serde_json::json!({}));
    let message = format!("{} {shelf_reason}", first["message"].as_str().unwrap_or(""));
    serde_json::json!({
        "message": message,
        "filters": first["filters"],
        "per_radiator": plans,
        "baffle_step": shelf_filter(corner, if shelf_gain > 0.0 { shelf_gain } else { 6.0 }, &shelf_reason)
    })
}

fn shelf_filter(corner_hz: f64, gain_db: f64, reason: &str) -> Value {
    serde_json::json!({
        "kind": "biquad",
        "type": "lowshelf",
        "f_hz": corner_hz,
        "q": 0.707,
        "gain_db": gain_db,
        "from_hz": corner_hz,
        "from_q": 0.707,
        "reason": reason,
        "correction": true
    })
}

fn shelf_db(gain_db: f64, corner_hz: f64, freq_hz: f64, design: &DesignFile) -> f64 {
    seatlock_correct::filter_db(
        &seatlock_correct::CorrectionFilter {
            filter_type: "lowshelf".to_string(),
            f_hz: corner_hz,
            q: 0.707,
            gain_db,
            from_hz: corner_hz,
            from_q: 0.707,
            reason: String::new(),
        },
        freq_hz,
        design.dsp.sample_rate_hz as f64,
    )
}

#[allow(clippy::too_many_arguments)]
fn search_shelf(
    curve: &RadiatorCurve,
    freqs: &[f64],
    level_db: f64,
    corner_hz: f64,
    design: &DesignFile,
    fixed: &[f64],
    protected: [f64; 2],
    alignment: (f64, f64),
    vented: bool,
    tuning_hz: f64,
    f6_max: f64,
    flat_max: f64,
) -> (f64, String) {
    let sample_rate = design.dsp.sample_rate_hz as f64;
    let mut best_gain = 0.0;
    let mut best_short = f64::MAX;
    for step in 0..=12 {
        let gain = step as f64 * 0.5;
        let spl: Vec<f64> = curve
            .spl_db
            .iter()
            .zip(freqs)
            .map(|(level, freq)| *level + shelf_db(gain, corner_hz, *freq, design))
            .collect();
        let max_for_eq: Vec<f64> = curve
            .max_spl_db
            .iter()
            .zip(freqs)
            .map(|(level, freq)| *level - shelf_db(gain, corner_hz, *freq, design))
            .collect();
        let outcome = seatlock_correct::correct(
            freqs,
            &spl,
            &max_for_eq,
            level_db,
            protected,
            design.dsp.correction.max_filters as usize,
            sample_rate,
            alignment.0,
            alignment.1,
            vented,
            tuning_hz,
            fixed,
        );
        let corrected: Vec<f64> = curve
            .spl_db
            .iter()
            .zip(freqs)
            .map(|(level, freq)| {
                let extra: f64 = outcome
                    .filters
                    .iter()
                    .map(|filter| seatlock_correct::filter_db(filter, *freq, sample_rate))
                    .sum();
                *level + shelf_db(gain, corner_hz, *freq, design) + extra
            })
            .collect();
        let (f6, flatness, loud) = seatlock_correct::metrics(freqs, &corrected, &curve.max_spl_db);
        let legal = corrected.iter().enumerate().all(|(index, level)| {
            if freqs[index] < f6 {
                return true;
            }
            let boost = level - curve.spl_db[index];
            let room = (curve.max_spl_db[index] - level_db).max(0.0);
            boost <= room + 0.3
        });
        if !legal {
            continue;
        }
        let shortfall = (f6 / f6_max.max(1.0)).log2().max(0.0)
            + (flatness - flat_max).max(0.0) / 1.5
            + (level_db - loud).max(0.0) / 6.0;
        if shortfall < best_short - 1e-4
            || (shortfall - best_short).abs() <= 1e-4 && gain >= best_gain
        {
            best_short = shortfall;
            best_gain = gain;
        }
    }
    let (f6, _, _) = seatlock_correct::metrics(freqs, &curve.spl_db, &curve.max_spl_db);
    let mut tight_index = 0usize;
    let mut tight_margin = f64::MAX;
    for (index, freq) in freqs.iter().enumerate() {
        if *freq < f6 {
            continue;
        }
        let boost = shelf_db(best_gain, corner_hz, *freq, design);
        let room = curve.max_spl_db[index] - level_db;
        let margin = room - boost;
        if margin < tight_margin {
            tight_margin = margin;
            tight_index = index;
        }
    }
    let name = tightest_limit(curve, tight_index, design);
    if best_gain >= 0.5 {
        (
            best_gain,
            format!("{name} limits the baffle-step shelf to {best_gain:.1} dB."),
        )
    } else {
        (
            0.0,
            format!("{name} stops the baffle-step shelf, so it is not applied."),
        )
    }
}

fn tightest_limit(curve: &RadiatorCurve, index: usize, design: &DesignFile) -> &'static str {
    let voltage = 2.83_f64;
    let travel = curve.cone_travel_m.get(index).copied().unwrap_or(0.0);
    let ohms = curve
        .impedance_ohm
        .get(index)
        .copied()
        .unwrap_or(1.0)
        .max(1e-6);
    let (xmax, re, pe) = first_driver_limits(design);
    let excursion = if travel > 1e-12 {
        xmax / travel
    } else {
        f64::MAX
    };
    let heat = if re > 0.0 {
        (pe / re).sqrt() / (voltage / ohms)
    } else {
        f64::MAX
    };
    let amp = amplifier_voltage_limit(design)
        .map(|limit| limit / voltage)
        .unwrap_or(f64::MAX);
    if excursion <= heat && excursion <= amp {
        "Cone travel"
    } else if heat <= amp {
        "Voice-coil heating"
    } else {
        "The amplifier"
    }
}

fn first_driver_limits(design: &DesignFile) -> (f64, f64, f64) {
    let Some(id) = design
        .speaker
        .radiators
        .first()
        .and_then(|radiator| radiator.fields.get("driver_id"))
        .and_then(Value::as_str)
    else {
        return (0.004, 6.0, 30.0);
    };
    let Some(record) = catalog::driver(id) else {
        return (0.004, 6.0, 30.0);
    };
    let number = |key: &str, fallback: f64| {
        record
            .pointer(&format!("/published/{key}"))
            .and_then(Value::as_f64)
            .unwrap_or(fallback)
    };
    (
        number("xmax_m", 0.004),
        number("re_ohm", 6.0),
        number("pe_w", 30.0),
    )
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
            enclosure: compare::ENCLOSURE.to_string(),
            radiation: "baffle".to_string(),
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
            enclosure: compare::ENCLOSURE.to_string(),
            radiation: "baffle".to_string(),
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
