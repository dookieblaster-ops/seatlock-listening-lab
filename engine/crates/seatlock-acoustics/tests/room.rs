//! CHK-ROOM-1, and the sound power and behind-the-speaker sound that the room estimate rests on
//! (README sections 5.6 and 5.10).

use num_complex::Complex64;
use seatlock_acoustics::{absorption_area_m2, spl_db};
use seatlock_core::constants::{C0, RHO0};
use seatlock_core::registry::{
    sound_power_by_directions, OuterShape, RadiationScene, Registry, RoomSound, SourcePatch,
};
use serde_json::{json, Map, Value};
use std::f64::consts::PI;

/// A flat square panel of side 0.3 m in the plane y = 0, facing the listener.
struct Panel;

impl OuterShape for Panel {
    fn interior_volume_m3(&self) -> Result<f64, String> {
        Ok(0.01)
    }
    fn baffle_width_m(&self) -> f64 {
        0.3
    }
    fn surface_facing(&self, outward: [f64; 3]) -> Option<String> {
        (outward[1] < -0.5).then(|| "front".to_string())
    }
    fn surface_outline(&self, surface: &str) -> Result<Vec<[f64; 3]>, String> {
        if surface != "front" {
            return Err("no such surface".into());
        }
        Ok(vec![
            [0.0, 0.0, 0.0],
            [0.3, 0.0, 0.0],
            [0.3, 0.0, 0.3],
            [0.0, 0.0, 0.3],
        ])
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    seatlock_acoustics::register(&mut registry);
    registry
}

fn fields(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn scene<'a>(shape: &'a Panel, space: &'a str) -> RadiationScene<'a> {
    RadiationScene {
        shape,
        space,
        baffle_width_m: 0.3,
        reference_m: [0.15, 0.0, 0.15],
    }
}

fn center_patch() -> SourcePatch {
    SourcePatch::aperture([0.15, 0.0, 0.15], [0.0, -1.0, 0.0], PI * 0.01 * 0.01)
}

/// Volume velocity of a simple source that radiates 1 W into all directions.
fn one_watt(freq_hz: f64) -> Complex64 {
    let k = 2.0 * PI * freq_hz / C0;
    Complex64::new((4.0 * PI / (RHO0 * C0 * k * k)).sqrt(), 0.0)
}

fn no_progress() -> impl FnMut(usize, usize) -> bool {
    |_, _| true
}

#[test]
fn chk_room_1_diffuse_and_direct_levels_of_a_one_watt_source() {
    let registry = registry();
    let room = registry
        .create_room(
            "simple",
            &fields(json!({
                "wall_behind_m": 0.5, "wall_reflection": 0.0, "volume_m3": 50.0, "rt60_s": 0.4
            })),
        )
        .unwrap();
    assert!((absorption_area_m2(50.0, 0.4) - 20.12).abs() < 0.01);

    let radiation = registry.create_radiation("point", &Map::new()).unwrap();
    let shape = Panel;
    let scene = scene(&shape, "full");
    let freqs = [100.0, 1000.0, 5000.0];
    let patches = [center_patch()];
    let velocity = vec![freqs.iter().map(|f| one_watt(*f)).collect::<Vec<_>>()];
    let power = radiation
        .sound_power(&scene, &patches, &velocity, &freqs, &mut no_progress())
        .unwrap();
    for watts in &power.power_w {
        assert!((watts - 1.0).abs() < 1e-9, "{watts} W");
    }
    let listener = [0.15, -2.5, 0.15];
    let transfer = radiation
        .transfer(&scene, &patches, &freqs, listener, &mut no_progress())
        .unwrap();
    let direct: Vec<Complex64> = (0..freqs.len())
        .map(|bin| transfer.per_source[0][bin] * velocity[0][bin])
        .collect();
    for pressure in &direct {
        assert!((spl_db(*pressure) - 101.19).abs() < 0.05);
    }
    // No wall reflection: the room asks for no image of the listening position.
    assert!(room.images(listener, [0.3, 0.2, 0.3]).is_empty());
    assert!(room.needs_sound_power());
    let in_room = room
        .in_room(&RoomSound {
            freqs_hz: &freqs,
            direct: &direct,
            images: &[],
            power_w: &power.power_w,
        })
        .unwrap();
    for (diffuse, reflection) in in_room.diffuse_pa.iter().zip(&in_room.reflection) {
        assert!((spl_db(Complex64::new(*diffuse, 0.0)) - 113.12).abs() < 0.05);
        assert_eq!(reflection.norm(), 0.0);
    }
}

#[test]
fn the_wall_behind_mirrors_the_listening_position_and_scales_the_sound() {
    let registry = registry();
    let room = registry
        .create_room(
            "simple",
            &fields(json!({
                "wall_behind_m": 0.5, "wall_reflection": 0.9, "volume_m3": 50.0, "rt60_s": 0.4
            })),
        )
        .unwrap();
    // Cabinet 0.2 m deep: the wall is at y = 0.7, so a listener at y = -1 is mirrored to 2.4.
    let images = room.images([0.15, -1.0, 0.15], [0.3, 0.2, 0.3]);
    assert_eq!(images.len(), 1);
    assert!((images[0].point_m[1] - 2.4).abs() < 1e-12);
    assert_eq!(images[0].point_m[0], 0.15);
    assert_eq!(images[0].gain, 0.9);
    let freqs = [200.0];
    let direct = [Complex64::new(1.0, 0.0)];
    let image = vec![vec![Complex64::new(0.0, 0.5)]];
    let in_room = room
        .in_room(&RoomSound {
            freqs_hz: &freqs,
            direct: &direct,
            images: &image,
            power_w: &[0.0],
        })
        .unwrap();
    assert!((in_room.reflection[0] - Complex64::new(0.0, 0.45)).norm() < 1e-12);
    assert_eq!(in_room.diffuse_pa[0], 0.0);
}

#[test]
fn a_room_with_missing_or_impossible_numbers_is_refused() {
    let registry = registry();
    for bad in [
        json!({ "wall_behind_m": 0.5, "wall_reflection": 0.9, "volume_m3": 50.0 }),
        json!({ "wall_behind_m": 0.5, "wall_reflection": 0.9, "volume_m3": 0.0, "rt60_s": 0.4 }),
        json!({ "wall_behind_m": -1.0, "wall_reflection": 0.9, "volume_m3": 50.0, "rt60_s": 0.4 }),
        json!({ "wall_behind_m": 0.5, "wall_reflection": 1.5, "volume_m3": 50.0, "rt60_s": 0.4 }),
    ] {
        assert!(registry.create_room("simple", &fields(bad)).is_err());
    }
}

#[test]
fn point_sources_radiate_twice_the_power_into_half_space() {
    let registry = registry();
    let radiation = registry.create_radiation("point", &Map::new()).unwrap();
    let shape = Panel;
    let freqs = [50.0, 500.0, 5000.0];
    let patches = [center_patch()];
    let velocity = vec![freqs.iter().map(|f| one_watt(*f)).collect::<Vec<_>>()];
    for (space, expected) in [("full", 1.0), ("half", 2.0)] {
        let scene = scene(&shape, space);
        let power = radiation
            .sound_power(&scene, &patches, &velocity, &freqs, &mut no_progress())
            .unwrap();
        // The far-field integral of the model's own pressure agrees with its closed form.
        let integral = sound_power_by_directions(
            radiation.as_ref(),
            &scene,
            &patches,
            &velocity,
            &freqs,
            &mut no_progress(),
        )
        .unwrap();
        for (closed, summed) in power.power_w.iter().zip(&integral.power_w) {
            assert!((closed - expected).abs() < 1e-9, "{space}: {closed} W");
            assert!((summed / closed - 1.0).abs() < 0.05, "{space}: {summed} W");
        }
        // Half space sends nothing behind the baffle; full space sends the same both ways.
        let behind = radiation
            .transfer(
                &scene,
                &patches,
                &freqs,
                [0.15, 2.0, 0.15],
                &mut no_progress(),
            )
            .unwrap();
        let front = radiation
            .transfer(
                &scene,
                &patches,
                &freqs,
                [0.15, -2.0, 0.15],
                &mut no_progress(),
            )
            .unwrap();
        let ratio = behind.per_source[0][1].norm() / front.per_source[0][1].norm();
        assert!((ratio - if space == "full" { 1.0 } else { 0.0 }).abs() < 1e-12);
    }
}

#[test]
fn the_baffle_model_reaches_behind_the_panel_only_at_low_frequency() {
    let registry = registry();
    let radiation = registry.create_radiation("baffle", &Map::new()).unwrap();
    let shape = Panel;
    let scene = scene(&shape, "half");
    let patches = [center_patch()];
    let freqs = [20.0, 8000.0];
    let front = radiation
        .transfer(
            &scene,
            &patches,
            &freqs,
            [0.6, -3.0, 0.4],
            &mut no_progress(),
        )
        .unwrap();
    // Off the panel's axis: straight behind the middle, the edge's sound arrives nearly in step
    // (the bright spot behind a disc), which is not what this test is about.
    let behind = radiation
        .transfer(
            &scene,
            &patches,
            &freqs,
            [0.6, 3.0, 0.4],
            &mut no_progress(),
        )
        .unwrap();
    // At 20 Hz the panel is far smaller than the wavelength: the same level all around,
    // that of a simple source in full space.
    let k = 2.0 * PI * 20.0 / C0;
    let range = (0.45_f64 * 0.45 + 9.0 + 0.25 * 0.25).sqrt();
    let full_space = k * C0 * RHO0 / (4.0 * PI * range);
    assert!((front.per_source[0][0].norm() / full_space - 1.0).abs() < 0.01);
    assert!((behind.per_source[0][0].norm() / full_space - 1.0).abs() < 0.01);
    // At 8 kHz the panel shadows its back: at least 12 dB down.
    let shadow = behind.per_source[0][1].norm() / front.per_source[0][1].norm();
    assert!(shadow < 0.25, "{shadow}");

    // Sound power: a simple source in full space at low frequency (1 W for this velocity).
    let low = [20.0, 40.0];
    let velocity = vec![low.iter().map(|f| one_watt(*f)).collect::<Vec<_>>()];
    let power = radiation
        .sound_power(&scene, &patches, &velocity, &low, &mut no_progress())
        .unwrap();
    for watts in &power.power_w {
        assert!((watts - 1.0).abs() < 0.03, "{watts} W");
    }
}
