//! The makers' measured on-axis curves (README 5.6, 9.1; owner 2026-11-12: download from
//! respected sources).

use seatlock_core::catalog;
use seatlock_sim::{builtin, simulate, SimulateRequest};
use serde_json::{json, Value};

fn curve(id: &str) -> Vec<(f64, f64)> {
    let record = catalog::driver(id).unwrap();
    let path = record["response_file"].as_str().unwrap();
    let file: Value = serde_json::from_str(catalog::embedded_file(path).unwrap()).unwrap();
    file["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["hz"].as_f64().unwrap(), p["db"].as_f64().unwrap()))
        .collect()
}

#[test]
fn every_response_file_is_sourced_and_is_not_mistaken_for_a_driver() {
    let drivers = catalog::all("catalog/drivers/");
    assert!(drivers.iter().all(|d| d.get("id").is_some()));
    let mut with_curve = 0;
    for driver in &drivers {
        let Some(path) = driver.get("response_file").and_then(Value::as_str) else {
            continue;
        };
        with_curve += 1;
        let file: Value = serde_json::from_str(catalog::embedded_file(path).unwrap()).unwrap();
        assert_eq!(file["driver_id"], driver["id"], "{path}");
        assert!(file["url"].as_str().unwrap().starts_with("https://"));
        assert!(file["read_on"].as_str().is_some() && file["revision"].as_str().is_some());
        let points = curve(driver["id"].as_str().unwrap());
        assert!(points.len() >= 100, "{path}");
        assert!(points.windows(2).all(|w| w[1].0 > w[0].0), "{path}");
        assert!(points.last().unwrap().0 > 15000.0, "{path}");
    }
    assert_eq!(with_curve, 12);
}

#[test]
fn straight_ahead_the_level_follows_the_makers_curve_above_the_blend() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../designs/examples/pvc-tube.seatlock.json"
    );
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value["dsp"]["per_radiator"][0]["blocks"] = json!([]);
    value["dsp"]["whole_speaker"] = json!([]);
    let design = serde_json::from_value(value).unwrap();
    let result = simulate(&design, &SimulateRequest::default(), &builtin());
    assert!(result
        .fidelity
        .ignored
        .iter()
        .any(|line| line.starts_with("measured above")));
    let measured = curve("dayton-tcp115-4");
    let at = |f: f64| {
        let i = result.frequencies_hz.iter().position(|x| *x >= f).unwrap();
        (result.frequencies_hz[i], result.summed_spl_db[i])
    };
    let interp = |f: f64| {
        let w = measured
            .windows(2)
            .find(|w| f >= w[0].0 && f <= w[1].0)
            .unwrap();
        let t = (f / w[0].0).ln() / (w[1].0 / w[0].0).ln();
        w[0].1 + t * (w[1].1 - w[0].1)
    };
    // The TCP115-4's piston hands over near 2.4 kHz; 5.65 kHz is its deepest dip.
    let (f1, l1) = at(3000.0);
    for f in [4000.0, 5650.0, 8000.0, 12000.0] {
        let (f2, l2) = at(f);
        let model = l2 - l1;
        let maker = interp(f2) - interp(f1);
        assert!(
            (model - maker).abs() < 0.01,
            "{f2} Hz: {model} against {maker}"
        );
    }
}
