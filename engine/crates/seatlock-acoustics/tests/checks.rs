use seatlock_acoustics::{
    derive_mechanical, piston_directivity, reference_half_space_spl_db, sealed_alignment,
    sealed_point, vented_point, PortSpec, Published,
};
use seatlock_core::grid::log_grid;

fn woofer_a() -> Published {
    Published {
        fs_hz: 50.0,
        qes: 0.45,
        qms: 4.0,
        vas_m3: 0.010,
        re_ohm: 6.0,
        le_h: 0.0005,
        sd_m2: 0.0085,
        xmax_m: 0.004,
        pe_w: 30.0,
    }
}

fn woofer_b() -> Published {
    Published {
        fs_hz: 40.0,
        qes: 0.4144,
        qms: 5.0,
        vas_m3: 0.020,
        re_ohm: 6.0,
        le_h: 0.0,
        sd_m2: 0.013,
        xmax_m: 0.006,
        pe_w: 50.0,
    }
}

fn close_rel(got: f64, expected: f64, relative: f64) {
    let tolerance = expected.abs() * relative;
    assert!(
        (got - expected).abs() <= tolerance,
        "{got} vs {expected} (relative {relative})"
    );
}

#[test]
fn chk_drv_1_mechanical_parameters() {
    let a = derive_mechanical(&woofer_a());
    close_rel(a.cms, 9.7598e-4, 0.001);
    close_rel(a.mms, 0.010381, 0.001);
    close_rel(a.rms, 0.8154, 0.001);
    close_rel(a.bl, 6.594, 0.001);
    close_rel(a.qts, 0.4045, 0.001);

    let b = derive_mechanical(&woofer_b());
    close_rel(b.cms, 8.3449e-4, 0.001);
    close_rel(b.mms, 0.018971, 0.001);
    close_rel(b.rms, 0.9536, 0.001);
    close_rel(b.bl, 8.309, 0.001);
    close_rel(b.qts, 0.3827, 0.001);
}

#[test]
fn chk_drv_2_reference_efficiency_at_1khz() {
    let mut published = woofer_a();
    published.le_h = 0.0;
    let (spl, _, _) = sealed_point(&published, 0.005, 2.83, 1_000.0, "half", 1.0, 0.16);
    let expected = reference_half_space_spl_db(&published, 2.83, 1.0);
    assert!(
        (expected - 87.75).abs() < 0.02,
        "formula {expected} is not the published 87.75"
    );
    assert!(
        (spl - expected).abs() <= 0.02,
        "spl {spl} vs formula {expected}"
    );
}

#[test]
fn chk_box_1_sealed_highpass() {
    let mut published = woofer_a();
    published.le_h = 0.0;
    let (fc, qtc) = sealed_alignment(&published, 0.005);
    assert!((fc - 86.60).abs() < 0.01, "fc {fc}");
    assert!((qtc - 0.7006).abs() < 0.0001, "qtc {qtc}");
    let level = reference_half_space_spl_db(&published, 2.83, 1.0);
    for freq in log_grid(5.0, 20_000.0, 24.0) {
        let (spl, _, _) = sealed_point(&published, 0.005, 2.83, freq, "half", 1.0, 0.16);
        let s = num_complex_s(freq, fc);
        let highpass = (s * s) / (s * s + s / qtc + 1.0);
        let expected = level + 20.0 * highpass.norm().log10();
        assert!(
            (spl - expected).abs() <= 0.001,
            "{freq} Hz: {spl} vs {expected}"
        );
    }
}

fn chk_box_3_port() -> PortSpec {
    let radius = 0.0175;
    PortSpec {
        area_m2: std::f64::consts::PI * radius * radius,
        length_eff_m: 0.12686,
    }
}

#[test]
fn chk_box_3_vented_butterworth() {
    let published = woofer_b();
    let volume = 0.020 / 2.0_f64.sqrt();
    let level = reference_half_space_spl_db(&published, 2.83, 1.0);
    assert!((level - 88.21).abs() < 0.02, "level {level}");
    let (low_spl, _, _, _, h) = vented_point(
        &published,
        volume,
        chk_box_3_port(),
        2.83,
        1.0,
        "half",
        1.0,
        0.2,
    );
    assert!((h - 1.0).abs() < 0.01, "H_port at 1 Hz is {h}");
    let _ = low_spl;
    for freq in log_grid(8.0, 800.0, 24.0) {
        let (spl, _, _, _, _) = vented_point(
            &published,
            volume,
            chk_box_3_port(),
            2.83,
            freq,
            "half",
            1.0,
            0.2,
        );
        let ratio = 40.0 / freq;
        let magnitude = 1.0 / (1.0 + ratio.powi(8)).sqrt();
        let expected = level + 20.0 * magnitude.max(1e-30).log10();
        assert!(
            (spl - expected).abs() <= 0.001,
            "{freq} Hz: {spl} vs {expected}"
        );
    }
}

#[test]
fn chk_box_4_vented_samples() {
    let published = woofer_b();
    let volume = 0.020 / 2.0_f64.sqrt();
    let cases = [
        (20.0, 64.11, 3.462e-3, 7.84),
        (40.0, 85.20, 0.0005e-3, 11.11),
        (80.0, 88.19, 0.866e-3, 1.96),
    ];
    for (freq, spl_db, excursion_m, speed) in cases {
        let (spl, impedance, excursion, port, _) = vented_point(
            &published,
            volume,
            chk_box_3_port(),
            2.83,
            freq,
            "half",
            1.0,
            0.2,
        );
        assert!((spl - spl_db).abs() <= 0.02, "{freq} spl {spl}");
        if freq == 40.0 {
            assert!((impedance - 6.0).abs() <= 0.03, "z {impedance}");
            assert!(excursion < 0.001e-3, "x {excursion}");
        } else {
            let travel_tol = excursion_m * 0.005;
            assert!(
                (excursion - excursion_m).abs() <= travel_tol.max(1e-6),
                "{freq} x {excursion} vs {excursion_m}"
            );
        }
        let speed_tol = speed * 0.005;
        assert!(
            (port - speed).abs() <= speed_tol,
            "{freq} port {port} vs {speed}"
        );
    }
    let peak_near = |low: f64, high: f64| {
        let mut best_f = low;
        let mut best_z = 0.0;
        let mut freq = low;
        while freq <= high {
            let (_, impedance, _, _, _) = vented_point(
                &published,
                volume,
                chk_box_3_port(),
                2.83,
                freq,
                "half",
                1.0,
                0.2,
            );
            if impedance > best_z {
                best_z = impedance;
                best_f = freq;
            }
            freq += 0.02;
        }
        best_f
    };
    let low_peak = peak_near(18.0, 30.0);
    let high_peak = peak_near(60.0, 85.0);
    assert!((low_peak - 22.8).abs() <= 0.2, "low peak {low_peak}");
    assert!((high_peak - 70.4).abs() <= 0.2, "high peak {high_peak}");
}

#[test]
fn chk_wg_1_closed_duct_impedance() {
    let area = 0.001;
    let zero = seatlock_acoustics::closed_duct_impedance(0.5, area, 171.6);
    let pole = seatlock_acoustics::closed_duct_impedance(0.5, area, 343.2);
    let zero_3 = seatlock_acoustics::closed_duct_impedance(0.5, area, 514.8);
    assert!(zero.norm() < 1.0, "171.6 Hz {}", zero.norm());
    assert!(pole.norm() > 1.0e6, "343.2 Hz {}", pole.norm());
    assert!(zero_3.norm() < 1.0, "514.8 Hz {}", zero_3.norm());
}

#[test]
fn chk_wg_2_helmholtz_matches_lumped() {
    let volume = 0.002;
    let radius = 0.010;
    let length = 0.030;
    let mut best_f = 0.0;
    let mut best = 0.0;
    let mut freq = 90.0;
    while freq < 115.0 {
        let result = seatlock_acoustics::helmholtz(volume, radius, length, freq);
        if result.z_in.norm() > best {
            best = result.z_in.norm();
            best_f = freq;
        }
        freq += 0.05;
    }
    assert!(
        (best_f - 102.5).abs() / 102.5 <= 0.005,
        "resonance {best_f}"
    );
}

#[test]
fn chk_wg_3_ported_box_matches_lumped_and_shows_the_pipe() {
    let published = woofer_b();
    let volume = 0.020 / 2.0_f64.sqrt();
    let radius = 0.0175;
    let physical = 0.10127;
    let mech = derive_mechanical(&published);
    for freq in [20.0, 40.0, 80.0, 160.0] {
        let lumped = vented_point(
            &published,
            volume,
            PortSpec {
                area_m2: std::f64::consts::PI * radius * radius,
                length_eff_m: physical + (0.849 + 0.613) * radius,
            },
            2.83,
            freq,
            "half",
            1.0,
            0.2,
        );
        let wave = seatlock_acoustics::ported_box(volume, radius, physical, freq);
        let terminal = seatlock_acoustics::terminal_at(&published, &mech, wave.z_in, 2.83, freq);
        let h = wave.openings.first().copied().unwrap_or_default();
        let into_room = terminal.volume_velocity * (num_complex::Complex64::new(1.0, 0.0) - h);
        let pressure =
            seatlock_acoustics::pressure_from_volume_velocity(into_room, freq, 1.0, "half", 0.2);
        let spl = seatlock_acoustics::spl_db(pressure);
        assert!(
            (spl - lumped.0).abs() <= 0.1,
            "{freq} Hz waveguide {spl} lumped {}",
            lumped.0
        );
    }
    let mut best_f = 0.0;
    let mut best = 0.0;
    let mut freq = 1100.0;
    while freq < 1600.0 {
        let wave = seatlock_acoustics::ported_box(volume, radius, physical, freq);
        let mag = wave
            .openings
            .first()
            .map(|value| value.norm())
            .unwrap_or(0.0);
        if mag > best {
            best = mag;
            best_f = freq;
        }
        freq += 1.0;
    }
    assert!(
        (best_f - 1355.0).abs() / 1355.0 <= 0.02,
        "pipe peak {best_f}"
    );
}

#[test]
fn chk_rad_1_piston_directivity() {
    let on = piston_directivity(0.0);
    let at_30 = piston_directivity(3.0 * 30.0_f64.to_radians().sin());
    let at_60 = piston_directivity(3.0 * 60.0_f64.to_radians().sin());
    let db30 = 20.0 * (at_30.abs() / on).log10();
    let db60 = 20.0 * (at_60.abs() / on).log10();
    assert!((db30 - -2.57).abs() <= 0.1, "30 deg {db30}");
    assert!((db60 - -8.81).abs() <= 0.1, "60 deg {db60}");
}

#[test]
fn chk_rad_2_circular_baffle_on_axis() {
    use seatlock_acoustics::{circular_outline, diffraction_factor};
    let outline = circular_outline([0.0, 0.0, 0.0], 0.10, [0.0, -1.0, 0.0], 128);
    let listener = [0.0, -100.0, 0.0];
    let source = [0.0, 0.0, 0.0];
    let cases = [(10.0, -6.02), (1716.0, 3.52), (3432.0, -6.02)];
    for (freq, expected) in cases {
        let factor = diffraction_factor(source, listener, &outline, freq);
        let db = 20.0 * factor.norm().log10();
        assert!((db - expected).abs() <= 0.05, "{freq} Hz {db}");
    }
}

#[test]
fn sensitivity_inverts_chk_drv_2_back_to_vas() {
    let published = woofer_a();
    let exact = reference_half_space_spl_db(&published, 2.83, 1.0);
    let mut record = seatlock_core::catalog::driver("test-woofer-a").unwrap();
    record["published"]
        .as_object_mut()
        .unwrap()
        .remove("vas_m3");
    record["published"]["sens_db_2v83"] = serde_json::json!(exact);
    let built = seatlock_acoustics::published_from_catalog(&record).unwrap();
    assert!(
        (built.vas_m3 - 0.01).abs() / 0.01 < 0.0001,
        "exact {}",
        built.vas_m3
    );
    record["published"]["sens_db_2v83"] = serde_json::json!(87.75);
    let rounded = seatlock_acoustics::published_from_catalog(&record).unwrap();
    assert!(
        (rounded.vas_m3 - 0.01).abs() / 0.01 < 0.001,
        "rounded {}",
        rounded.vas_m3
    );
    assert!(seatlock_acoustics::derivation_note(&record)
        .unwrap()
        .contains("sensitivity"));
}

#[test]
fn sheet_drawings_supply_mounting_sizes() {
    // Read from the maker's drawings on 2026-11-15 (mm on the sheet).
    let nd = seatlock_core::catalog::driver("dayton-nd25fa-4").unwrap();
    assert_eq!(
        nd.pointer("/mounting/outer_diameter_m")
            .and_then(|v| v.as_f64()),
        Some(0.066)
    );
    assert_eq!(
        nd.pointer("/mounting/cutout_diameter_m")
            .and_then(|v| v.as_f64()),
        Some(0.045)
    );
    assert_eq!(
        nd.pointer("/mounting/bolt_count").and_then(|v| v.as_u64()),
        Some(4)
    );
    let rst = seatlock_core::catalog::driver("dayton-rst28f-4").unwrap();
    assert_eq!(
        rst.pointer("/mounting/outer_diameter_m")
            .and_then(|v| v.as_f64()),
        Some(0.1045)
    );
    assert_eq!(
        rst.pointer("/mounting/cutout_diameter_m")
            .and_then(|v| v.as_f64()),
        Some(0.072)
    );
    assert_eq!(
        rst.pointer("/mounting/bolt_count").and_then(|v| v.as_u64()),
        Some(4)
    );
    let rs = seatlock_core::catalog::driver("dayton-rs225-4").unwrap();
    assert_eq!(
        rs.pointer("/mounting/cutout_diameter_m")
            .and_then(|v| v.as_f64()),
        Some(0.184)
    );
    assert_eq!(
        rs.pointer("/mounting/bolt_circle_m")
            .and_then(|v| v.as_f64()),
        Some(0.209)
    );
    assert_eq!(
        rs.pointer("/mounting/depth_m").and_then(|v| v.as_f64()),
        Some(0.092)
    );
    // The DC28F-8's sizes were read on 2026-11-12 from the seller's listing as a search
    // extract, so they are stored and marked unconfirmed; its bolt circle is still unknown.
    let dc28f = seatlock_core::catalog::driver("dayton-dc28f-8").unwrap();
    assert!(dc28f.pointer("/mounting/cutout_diameter_m").is_some());
    assert_eq!(
        dc28f
            .pointer("/mounting/source/via")
            .and_then(|v| v.as_str()),
        Some("search_extract")
    );
    assert!(dc28f.pointer("/mounting/bolt_count").is_none());
    let nd65 = seatlock_core::catalog::driver("dayton-nd65-4").unwrap();
    assert!(nd65.pointer("/mounting/bolt_circle_m").is_none());
}

#[test]
fn a_measured_curve_sets_the_shape_above_ka_equals_one() {
    use num_complex::Complex64;
    use seatlock_acoustics::blend_measured_curve;
    use seatlock_core::constants::C0;
    let blend_hz = 849.0;
    let radius = C0 / (2.0 * std::f64::consts::PI * blend_hz);
    let mut pressure = vec![Complex64::new(1.0, 0.0); 3];
    let freqs = [400.0, blend_hz, blend_hz * 2.0];
    let label = blend_measured_curve(
        &mut pressure,
        &freqs,
        radius,
        &[(100.0, 80.0), (blend_hz, 80.0), (blend_hz * 2.0, 86.0)],
    );
    assert!(label.starts_with("measured"));
    assert!((pressure[0].norm() - 1.0).abs() < 1e-9);
    assert!(
        (pressure[2].norm() - 2.0).abs() < 0.05,
        "{}",
        pressure[2].norm()
    );
    let mut bare = vec![Complex64::new(1.0, 0.0); 1];
    let bare_label = blend_measured_curve(&mut bare, &[1000.0], radius, &[]);
    assert!(bare_label.contains("no measured cone break-up"));
    assert!((bare[0].norm() - 1.0).abs() < 1e-12);
}

#[test]
fn a_sheet_without_vas_uses_the_force_factor_when_it_is_printed() {
    let record = serde_json::json!({
        "published": {
            "fs_hz": 1000.0, "qes": 1.0, "qms": 2.0, "re_ohm": 6.0, "le_h": 0.0001,
            "sd_m2": 0.001, "pe_w": 10.0, "bl_tm": 3.0
        }
    });
    let published = seatlock_acoustics::published_from_catalog(&record).unwrap();
    let mech = derive_mechanical(&published);
    let omega = 2.0 * std::f64::consts::PI * 1000.0;
    let expected_mms = 9.0 / (omega * 6.0);
    assert!((mech.mms - expected_mms).abs() / expected_mms < 0.01);
    assert!(seatlock_acoustics::derivation_note(&record)
        .unwrap()
        .contains("force factor"));
}

#[test]
fn nd25fa4_is_derived_from_sensitivity_without_a_travel_limit() {
    let record = seatlock_core::catalog::driver("dayton-nd25fa-4").unwrap();
    let published = seatlock_acoustics::published_from_catalog(&record).unwrap();
    assert_eq!(published.xmax_m, 0.0);
    assert!(record.pointer("/published/xmax_m").is_none());
    assert!(seatlock_acoustics::derivation_note(&record)
        .unwrap()
        .contains("sensitivity"));
}

#[test]
fn sourced_drivers_match_printed_mechanics_or_would_warn() {
    for id in [
        "dayton-tcp115-4",
        "dayton-ps95-8",
        "dayton-rs100-8",
        "dayton-nd65-4",
        "dayton-dsa135-8",
        "dayton-pc105-8",
        "dayton-tcp115-8",
        "dayton-rs225-8",
    ] {
        let record = seatlock_core::catalog::driver(id).expect(id);
        assert_eq!(record["status"], "sourced");
        assert_eq!(
            record["tier"],
            if id.contains("rs225") {
                "reference"
            } else {
                "budget"
            }
        );
        assert_eq!(record["source"]["read_on"], "2026-11-10");
        assert_eq!(record["price"]["read_on"], "2026-11-10");
        let published = seatlock_acoustics::published_from_catalog(&record).unwrap();
        let mech = derive_mechanical(&published);
        let printed = &record["published"];
        match seatlock_acoustics::printed_disagreement(&record) {
            Some(note) => assert!(note.contains("10 percent"), "{id}: {note}"),
            None => {
                close_rel(mech.mms, printed["mms_kg"].as_f64().unwrap(), 0.1);
                close_rel(mech.cms, printed["cms_m_per_n"].as_f64().unwrap(), 0.1);
                close_rel(mech.bl, printed["bl_tm"].as_f64().unwrap(), 0.1);
            }
        }
    }
    {
        let id = "dayton-rs225-4";
        let record = seatlock_core::catalog::driver(id).expect(id);
        assert!(record["price"].is_null());
        let published = seatlock_acoustics::published_from_catalog(&record).unwrap();
        let mech = derive_mechanical(&published);
        let printed = &record["published"];
        match seatlock_acoustics::printed_disagreement(&record) {
            Some(note) => assert!(note.contains("10 percent"), "{id}: {note}"),
            None => {
                close_rel(mech.mms, printed["mms_kg"].as_f64().unwrap(), 0.1);
                close_rel(mech.cms, printed["cms_m_per_n"].as_f64().unwrap(), 0.1);
                close_rel(mech.bl, printed["bl_tm"].as_f64().unwrap(), 0.1);
            }
        }
    }
    for id in [
        "dayton-nd25fa-4",
        "dayton-nd25fw-4",
        "dayton-dc28f-8",
        "dayton-rst28f-4",
    ] {
        let record = seatlock_core::catalog::driver(id).expect(id);
        let published = seatlock_acoustics::published_from_catalog(&record).expect(id);
        assert_eq!(published.xmax_m, 0.0);
        assert!(seatlock_acoustics::derivation_note(&record)
            .unwrap()
            .contains("sensitivity"));
    }
    let pvc = seatlock_core::catalog::embedded_file("catalog/stock/charlotte-pvc-sch40-4in.json")
        .unwrap();
    let pvc: serde_json::Value = serde_json::from_str(pvc).unwrap();
    assert_eq!(pvc["density_kg_m3"], 1400.0);
    for path in [
        "catalog/stock/sakrete-form-tube-8in.json",
        "catalog/stock/komatex-foam-board.json",
        "catalog/stock/ultrastock-mdf-3-4.json",
        "catalog/stock/sandeply-18mm.json",
    ] {
        let text = seatlock_core::catalog::embedded_file(path).expect(path);
        let record: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(record["density_kg_m3"].is_null(), "{path}");
        assert!(record["price"]["amount"].as_f64().unwrap() > 0.0, "{path}");
    }
}

fn num_complex_s(freq: f64, fc: f64) -> num_complex::Complex64 {
    num_complex::Complex64::new(0.0, freq / fc)
}

#[test]
fn chk_box_2_published_driver_in_5l() {
    let published = woofer_a();
    let cases = [
        (40.0, 74.02, 7.87, 1.381e-3),
        (86.60, 84.66, 59.33, 1.003e-3),
        (1_000.0, 87.12, 6.50, 0.0100e-3),
    ];
    for (freq, spl_db, ohms, excursion_m) in cases {
        let (spl, impedance, excursion) =
            sealed_point(&published, 0.005, 2.83, freq, "half", 1.0, 0.16);
        assert!(
            (spl - spl_db).abs() <= 0.02,
            "{freq} Hz spl {spl} vs {spl_db}"
        );
        close_rel(impedance, ohms, 0.005);
        close_rel(excursion, excursion_m, 0.005);
    }
}

#[test]
fn a_lossy_closed_duct_has_the_closed_form_impedance_at_its_quarter_wave() {
    // README 5.4: k becomes k - j*alpha, alpha = 2.99e-5 * sqrt(f) / a. A duct closed at its
    // far end has Z_in = -j*Z0*cot((k - j*alpha)*L); where k*L = pi/2 that is Z0*tanh(alpha*L),
    // a pure resistance, where the lossless duct of CHK-WG-1 has zero.
    use seatlock_acoustics::{solve_lossy, AcousticNode};
    let (length, radius) = (0.5, 0.02);
    let area = std::f64::consts::PI * radius * radius;
    let freq = seatlock_core::constants::C0 / (4.0 * length);
    let duct = AcousticNode::Duct {
        length_m: length,
        area_in_m2: area,
        area_out_m2: area,
        added_length_m: 0.0,
        load: Box::new(AcousticNode::Rigid),
    };
    let z = solve_lossy(&duct, freq).z_in;
    let z0 = seatlock_core::constants::RHO0 * seatlock_core::constants::C0 / area;
    let alpha = 2.99e-5 * freq.sqrt() / radius;
    let expected = z0 * (alpha * length).tanh();
    assert!(
        (z.re - expected).abs() < 1e-4 * expected,
        "{} against {expected}",
        z.re
    );
    assert!(z.im.abs() < 1e-4 * expected, "{}", z.im);
    // And the wave that reaches the far end is smaller by e^(-alpha*L) each way: at the
    // half wave the lossless duct's impedance is unbounded, the lossy one's is Z0/tanh.
    let z = solve_lossy(&duct, 2.0 * freq).z_in;
    let expected = z0 / (alpha * 2f64.sqrt() * length).tanh();
    assert!(
        (z.norm() - expected).abs() < 1e-3 * expected,
        "{} against {expected}",
        z.norm()
    );
}
