//! Independent measurements beside the maker's data (README 9.1, owner 2026-11-12).

use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::{json, Value};

fn two_way(woofer: &str, crossover_hz: Option<f64>) -> seatlock_core::DesignFile {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/two-way-vented.seatlock.json"
    );
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value["speaker"]["radiators"][0]["driver_id"] = json!(woofer);
    value["speaker"]["radiators"][1]["driver_id"] = json!("dayton-dc28f-8");
    for chain in value["dsp"]["per_radiator"].as_array_mut().unwrap() {
        match crossover_hz {
            Some(f) => {
                for block in chain["blocks"].as_array_mut().unwrap() {
                    block["f_hz"] = json!(f);
                }
            }
            None => chain["blocks"] = json!([]),
        }
    }
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_measured_value_replaces_the_makers_and_the_note_names_both() {
    let record = seatlock_core::catalog::driver("dayton-dsa175-8").unwrap();
    let (merged, note) = seatlock_acoustics::with_independent(&record);
    assert_eq!(merged["published"]["xmax_m"].as_f64(), Some(0.0021));
    assert_eq!(record["published"]["xmax_m"].as_f64(), Some(0.0053));
    // Values the test did not give stay the maker's.
    assert_eq!(merged["published"]["fs_hz"], record["published"]["fs_hz"]);
    let note = note.unwrap();
    assert!(
        note.contains("usable cone travel 2.1 mm (maker 5.3 mm)"),
        "{note}"
    );
    assert!(note.contains("Voice Coil Test Bench"), "{note}");
    assert!(note.contains("7 dB at 4.7 kHz"), "{note}");
    assert_eq!(seatlock_acoustics::breakup(&record), Some((4700.0, 7.0)));
    // A record with no independent data comes back as it was.
    let plain = seatlock_core::catalog::driver("dayton-dsa135-8").unwrap();
    let (same, none) = seatlock_acoustics::with_independent(&plain);
    assert_eq!(same, plain);
    assert!(none.is_none());
}

#[test]
fn the_tebm_uses_erins_printed_parameters_and_names_the_sensitivity() {
    let record = seatlock_core::catalog::driver("tectonic-tebm35c10-4").unwrap();
    assert!(record
        .get("response_file")
        .and_then(Value::as_str)
        .is_none());
    let (merged, note) = seatlock_acoustics::with_independent(&record);
    assert_eq!(merged["published"]["fs_hz"].as_f64(), Some(146.0));
    assert_eq!(merged["published"]["vas_m3"].as_f64(), Some(0.000148));
    assert_eq!(merged["published"]["sens_db_2v83"].as_f64(), Some(77.0));
    // Maker sheet still has its own figures.
    assert_eq!(record["published"]["fs_hz"].as_f64(), Some(166.0));
    assert_eq!(record["published"]["sens_db_1w"].as_f64(), Some(80.0));
    let note = note.unwrap();
    assert!(note.contains("Erin's Audio Corner"), "{note}");
    assert!(
        note.contains("sensitivity at 2.83 V 77 dB (maker 80 dB at 1 W)"),
        "{note}"
    );
    assert!(note.contains("resonance 146 Hz (maker 166 Hz)"), "{note}");
    assert!(merged["published"].get("sens_db_1w").is_none());
    // No printed break-up peak height on that review.
    assert_eq!(seatlock_acoustics::breakup(&record), None);
    // Independent acoustic sensitivity scales radiated level; Vas stays for compliance.
    let (policy_note, scale) = seatlock_acoustics::sensitivity_level_policy(&merged);
    let policy_note = policy_note.expect("disagreement note");
    assert!(
        policy_note.contains("independent sensitivity"),
        "{policy_note}"
    );
    assert!(
        policy_note.contains("level follows the independent figure"),
        "{policy_note}"
    );
    assert!(
        (20.0 * scale.log10() + 4.0).abs() < 0.5,
        "scale {scale} should be about -4 dB"
    );
}

#[test]
fn the_measured_travel_sets_the_bass_limit() {
    // Where cone travel is the first limit, the clean level is the level at the drive voltage
    // raised until the cone moves 2.1 mm, the measured travel, not the maker's 5.3 mm.
    let design = two_way("dayton-dsa175-8", Some(2000.0));
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    let woofer = &result.radiators[0];
    let i = result
        .frequencies_hz
        .iter()
        .position(|x| *x >= 25.0)
        .unwrap();
    let from_travel = woofer.spl_db[i] + 20.0 * (0.0021 / woofer.cone_travel_m[i]).log10();
    let from_maker = woofer.spl_db[i] + 20.0 * (0.0053 / woofer.cone_travel_m[i]).log10();
    eprintln!(
        "25 Hz: clean {:.2} dB, travel-limited at 2.1 mm {:.2} dB, at 5.3 mm {:.2} dB",
        woofer.max_spl_db[i], from_travel, from_maker
    );
    assert!(woofer.max_spl_db[i] <= from_travel + 0.01);
    assert!(woofer.max_spl_db[i] < from_maker - 7.0);
}

#[test]
fn a_crossover_above_the_measured_breakup_is_warned_about() {
    let registry = builtin();
    let request = SimulateRequest::default();
    let warned = |design: &seatlock_core::DesignFile| {
        simulate(design, &request, &registry)
            .issues
            .iter()
            .any(|issue| issue.code == "measured_breakup")
    };
    assert!(warned(&two_way("dayton-dsa175-8", Some(4000.0))));
    assert!(!warned(&two_way("dayton-dsa175-8", Some(2300.0))));
    assert!(warned(&two_way("dayton-dsa175-8", None)));
    // A driver without independent data is not warned about.
    assert!(!warned(&two_way("dayton-dsa135-8", Some(4000.0))));
}
