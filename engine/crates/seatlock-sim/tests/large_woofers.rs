//! Larger budget woofers (README 2.6 / 9.1): 8 in. cone area for cleaner 50 Hz / 95 dB.

use seatlock_core::catalog;
use seatlock_sim::builtin;
use seatlock_sim::compare::score_as_is;

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Same filter the generator's `parts()` uses for budget woofers with a cut-out size.
fn budget_woofer_ids() -> Vec<String> {
    catalog::all("catalog/drivers/")
        .into_iter()
        .filter_map(|record| {
            if record.get("status")?.as_str()? != "sourced" {
                return None;
            }
            if record.get("tier")?.as_str()? != "budget" {
                return None;
            }
            if record.get("kind")?.as_str()? != "woofer" {
                return None;
            }
            let cut = record.pointer("/mounting/cutout_diameter_m")?.as_f64()?;
            let outer = record.pointer("/mounting/outer_diameter_m")?.as_f64()?;
            if cut <= 0.0 || outer <= 0.0 {
                return None;
            }
            Some(record.get("id")?.as_str()?.to_string())
        })
        .collect()
}

#[test]
fn budget_eight_inch_woofers_are_in_the_catalog_with_mounting() {
    for id in ["dayton-ds215-8", "dayton-dc200-8"] {
        let record = catalog::driver(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(record["status"], "sourced");
        assert_eq!(record["tier"], "budget");
        assert_eq!(record["kind"], "woofer");
        let sd = record["published"]["sd_m2"].as_f64().unwrap();
        assert!(
            sd > 0.02,
            "{id} cone area {sd} m² should be an 8 in class (~0.021)"
        );
        assert!(record["mounting"]["cutout_diameter_m"].as_f64().unwrap() > 0.17);
        assert!(record["mounting"]["outer_diameter_m"].as_f64().unwrap() > 0.2);
        assert!(record["price"]["amount"].as_f64().unwrap() > 0.0);
    }
}

#[test]
fn the_generator_pool_includes_the_new_eight_inch_budget_woofers() {
    let ids = budget_woofer_ids();
    assert!(
        ids.iter().any(|id| id == "dayton-ds215-8"),
        "generator pool missing DS215-8: {ids:?}"
    );
    assert!(
        ids.iter().any(|id| id == "dayton-dc200-8"),
        "generator pool missing DC200-8: {ids:?}"
    );
}

#[test]
fn a_sealed_ds215_box_reaches_deeper_bass_than_a_small_budget_woofer() {
    let registry = builtin();
    let (large, _) = score_as_is(&example("ds215-sealed"), &registry);
    assert!(large.problems.is_empty(), "{:?}", large.problems);
    let preference = large.preference.expect("preference on every score");
    assert!(preference.is_finite(), "{preference}");
    eprintln!(
        "ds215-sealed: f6 {:.0} Hz, clean {:.1} dB, preference {:.2}",
        large.f6_hz, large.max_spl_db, preference
    );

    let ds215 = catalog::driver("dayton-ds215-8").unwrap();
    let tcp = catalog::driver("dayton-tcp115-4").unwrap();
    let displace = |record: &serde_json::Value| {
        record["published"]["sd_m2"].as_f64().unwrap()
            * record["published"]["xmax_m"].as_f64().unwrap()
    };
    assert!(
        displace(&ds215) > displace(&tcp) * 2.0,
        "8 in displacement {:.0} cm³ vs TCP115 {:.0} cm³",
        displace(&ds215) * 1e6,
        displace(&tcp) * 1e6
    );

    // mdf-box is a small TCP115-4; the larger sealed box with DS215 reaches deeper bass.
    let (small_score, _) = score_as_is(&example("mdf-box"), &registry);
    eprintln!(
        "mdf-box TCP115-4: f6 {:.0} Hz, clean {:.1} dB",
        small_score.f6_hz, small_score.max_spl_db
    );
    assert!(
        large.f6_hz < small_score.f6_hz - 10.0,
        "8 in bass {} Hz vs small {} Hz",
        large.f6_hz,
        small_score.f6_hz
    );
}
