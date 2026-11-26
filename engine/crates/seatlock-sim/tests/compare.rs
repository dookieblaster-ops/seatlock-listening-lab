//! Cost beside score, cheap against reference, build methods, and the optimizer
//! (README sections 6.2, 9.2, 11).

use seatlock_core::catalog;
use seatlock_sim::builtin;
use seatlock_sim::compare::{
    build_methods, cheap_against_reference, score, score_as_is, Body, Target,
};
use seatlock_sim::optimize::{combos, optimize_combo, OptimizeRequest};

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn shortfall_ranks_by_preference_and_clean_level() {
    // README 11.2: preference deficit + clean-level deficit; bass/evenness live inside preference.
    let target = Target {
        preference_min: 5.0,
        f6_hz_max: 50.0,
        flatness_db_max: 1.5,
        max_spl_db_min: 95.0,
        budget_max: None,
    };
    assert!((target.shortfall(Some(5.0), 95.0) - 0.0).abs() < 1e-12);
    assert!((target.shortfall(Some(4.0), 95.0) - 1.0).abs() < 1e-12);
    assert!((target.shortfall(Some(5.0), 89.0) - 1.0).abs() < 1e-12);
    assert!(target.shortfall(Some(6.0), 100.0) == 0.0);
    assert!(target.shortfall(None, 95.0) >= 10.0);
}

#[test]
fn a_scored_design_reports_preference_as_the_ranked_metric() {
    let registry = builtin();
    let (as_is, _) = score_as_is(&example("sealed-test"), &registry);
    let preference = as_is
        .preference
        .expect("CTA-2034 preference on every score");
    assert!(preference.is_finite(), "{preference}");
    let target = Target {
        preference_min: 5.0,
        f6_hz_max: 50.0,
        flatness_db_max: 1.5,
        max_spl_db_min: 95.0,
        budget_max: None,
    };
    let expected = target.shortfall(Some(preference), as_is.max_spl_db);
    assert!(
        (as_is.shortfall - expected).abs() < 1e-9,
        "shortfall {} vs preference-based {}",
        as_is.shortfall,
        expected
    );
}

#[test]
fn correction_keeps_the_bass_of_a_speaker_that_cannot_reach_the_design_level() {
    // The TCP115-4 in its PVC tube reaches only about 91 dB in its midrange. Correction used to
    // push the bass limit to 783 Hz looking for 95 dB; now it shapes the bass for 91 dB.
    let registry = builtin();
    let (corrected, _) = score(&example("pvc-tube"), &registry);
    assert!(corrected.f6_hz < 150.0, "bass limit {}", corrected.f6_hz);
}

#[test]
fn a_woofer_far_above_its_crossover_does_not_set_the_systems_clean_level() {
    // Each radiator gets the input through its own crossover. Before, the woofer's own limit at
    // 16 kHz, without its low-pass, capped the two-way at 41 dB.
    let registry = builtin();
    let (as_is, _) = score_as_is(&example("two-way-vented"), &registry);
    assert!(as_is.max_spl_db > 85.0, "clean level {}", as_is.max_spl_db);
}

#[test]
fn the_reference_build_uses_reference_parts_and_the_same_air() {
    let registry = builtin();
    let design = example("mdf-box");
    let compared = cheap_against_reference(&design, &registry);
    let reference = compared.reference.expect("built");
    for radiator in &reference.design.speaker.radiators {
        let id = radiator.fields["driver_id"].as_str().unwrap();
        let record = catalog::driver(id).unwrap();
        assert_eq!(record["tier"], "reference", "{id}");
    }
    let amp = catalog::amplifier(&reference.design.electronics.amplifiers[0]).unwrap();
    assert_eq!(amp["tier"], "reference");
    let outer = &reference.design.speaker.enclosure.outer;
    assert_eq!(outer.kind, "sheet_box");
    let stock = catalog::stock(outer.fields["stock_id"].as_str().unwrap()).unwrap();
    assert_eq!(stock["tier"], "reference");
    let a = compared.cheap.score.interior_volume_m3.unwrap();
    let b = reference.score.interior_volume_m3.unwrap();
    assert!((a / b - 1.0).abs() < 0.005, "{a} {b}");
    assert!(reference.score.cost_usd > compared.cheap.score.cost_usd);
}

#[test]
fn every_build_method_keeps_the_air_volume() {
    let registry = builtin();
    let design = example("pvc-tube");
    let (now, _) = score_as_is(&design, &registry);
    let volume = now.interior_volume_m3.unwrap();
    let rows = build_methods(&design, &registry);
    let built: Vec<_> = rows.iter().filter_map(|row| row.as_ref().ok()).collect();
    assert!(built.len() >= 4, "{rows:?}");
    let kinds: Vec<&str> = built
        .iter()
        .map(|v| v.design.speaker.enclosure.outer.kind.as_str())
        .collect();
    assert!(kinds.contains(&"box") && kinds.contains(&"sheet_box") && kinds.contains(&"tube"));
    for variant in &built {
        let v = variant.score.interior_volume_m3.unwrap();
        assert!((v / volume - 1.0).abs() < 0.005, "{} {v}", variant.label);
        assert!(variant.score.cost_usd > 0.0, "{}", variant.label);
    }
    // A stock part with no published wall cannot be built, and says why.
    assert!(rows
        .iter()
        .any(|row| matches!(row, Err((_, why)) if why.contains("wall"))));
}

#[test]
fn the_level_3_optimizer_searches_free_cabinet_proportions() {
    use seatlock_sim::compare::{rebuild_to, rebuild_to_proportions};

    let registry = builtin();
    let design = example("sealed-test");
    let (now, _) = score_as_is(&design, &registry);
    let volume = now.interior_volume_m3.unwrap();
    let body = Body::of(&design);
    let locked = rebuild_to(&design, &body, volume, &registry).unwrap().0;
    let free = rebuild_to_proportions(&design, &body, volume, Some((2.4, 0.7)), &registry)
        .unwrap()
        .0;
    let ratio = |d: &seatlock_core::DesignFile| {
        let f = &d.speaker.enclosure.outer.fields;
        let w = f.get("width_m").and_then(|v| v.as_f64()).unwrap();
        let h = f.get("height_m").and_then(|v| v.as_f64()).unwrap();
        h / w
    };
    assert!(
        (ratio(&free) - ratio(&locked)).abs() > 0.3,
        "locked {} free {}",
        ratio(&locked),
        ratio(&free)
    );
    let free_vol = score_as_is(&free, &registry).0.interior_volume_m3.unwrap();
    assert!(
        (free_vol / volume - 1.0).abs() < 0.01,
        "{free_vol} vs {volume}"
    );

    let sealed = combos(&design)
        .into_iter()
        .find(|c| {
            !c.vented
                && c.driver_id.as_deref() == Some("test-woofer-a")
                && matches!(c.body, Some(Body::Printed { .. }))
        })
        .expect("sealed printed combo for the current driver");
    let result = optimize_combo(
        &design,
        &OptimizeRequest {
            seed: 11,
            combo: sealed.index,
            evaluations: 8,
        },
        &registry,
        &mut |_, _| true,
    )
    .unwrap();
    assert!(
        result.best[0]
            .variables
            .iter()
            .any(|(name, _)| name == "height / width"),
        "expected free proportions in the search: {:?}",
        result.best[0].variables
    );
    assert!(
        result.best[0]
            .variables
            .iter()
            .any(|(name, _)| name == "depth / width"),
        "{:?}",
        result.best[0].variables
    );
    // Tubes keep locked proportions (pipe diameter) and do not invent a printed alternate.
    let tube = example("pvc-tube");
    let tube_combos = combos(&tube);
    assert!(
        tube_combos
            .iter()
            .all(|c| matches!(c.body, Some(Body::Tube { .. }))),
        "tubes stay as the current body only: {:?}",
        tube_combos
            .iter()
            .map(|c| c.label.clone())
            .collect::<Vec<_>>()
    );
    let tube_combo = tube_combos.into_iter().next().unwrap();
    let tube_result = optimize_combo(
        &tube,
        &OptimizeRequest {
            seed: 2,
            combo: tube_combo.index,
            evaluations: 6,
        },
        &registry,
        &mut |_, _| true,
    )
    .unwrap();
    assert!(
        tube_result.best[0]
            .variables
            .iter()
            .all(|(name, _)| name != "height / width" && name != "depth / width"),
        "{:?}",
        tube_result.best[0].variables
    );
}

#[test]
fn the_level_3_optimizer_lists_nearby_catalog_drivers_for_a_single_cone() {
    let registry = builtin();
    let design = example("sealed-test");
    let all = combos(&design);
    // Sealed + vented × current/alt driver × current/alt build (printed + thick sheet).
    assert!(
        all.len() >= 4 && all.len() <= 6 && all.len().is_multiple_of(2),
        "unexpected combo count {}",
        all.len()
    );
    let ids: std::collections::BTreeSet<_> =
        all.iter().filter_map(|c| c.driver_id.as_deref()).collect();
    assert!(ids.contains("test-woofer-a"), "{ids:?}");
    assert_eq!(
        ids.len(),
        2,
        "expected one catalog alternative, got {ids:?}"
    );
    // Multi-way still keeps the user's pair (no driver expansion); builds still expand.
    let two_way = combos(&example("two-way-vented"));
    assert!(
        two_way.len() >= 4 && two_way.len() <= 8,
        "unexpected two-way combo count {}",
        two_way.len()
    );
    assert!(two_way.iter().all(|c| c.driver_id.is_none()));
    // A combo that names another driver actually installs it.
    let alt = all
        .iter()
        .find(|c| {
            c.driver_id
                .as_deref()
                .is_some_and(|id| id != "test-woofer-a")
                && !c.vented
                && c.body
                    .as_ref()
                    .is_some_and(|b| matches!(b, Body::Printed { .. }))
        })
        .expect("a sealed printed combo with an alternate driver");
    let result = optimize_combo(
        &design,
        &OptimizeRequest {
            seed: 3,
            combo: alt.index,
            evaluations: 4,
        },
        &registry,
        &mut |_, _| true,
    )
    .unwrap();
    assert!(
        result.notes.iter().any(|n| n.contains("Driver set to")),
        "{:?}",
        result.notes
    );
    let installed = result.best[0].design.speaker.radiators[0]
        .fields
        .get("driver_id")
        .and_then(serde_json::Value::as_str);
    assert_eq!(installed, alt.driver_id.as_deref());
}

#[test]
fn the_level_3_optimizer_tries_another_build_and_free_proportions() {
    let registry = builtin();
    let design = example("sealed-test");
    let all = combos(&design);
    // Current body × (current + nearby driver) × sealed/vented + alt body × current driver
    // × sealed/vented ≤ 6. Foam board is not an alt (≥ 9 mm sheets only).
    assert!(
        all.len() >= 4 && all.len() <= 6 && all.len().is_multiple_of(2),
        "unexpected combo count {}",
        all.len()
    );
    let sheet = all
        .iter()
        .find(|c| {
            !c.vented
                && c.driver_id.as_deref() == Some("test-woofer-a")
                && matches!(c.body, Some(Body::Sheet { .. }))
        })
        .expect("a sealed sheet-stock combo for the current driver");
    let sheet_stock = match &sheet.body {
        Some(Body::Sheet { stock_id }) => stock_id.as_str(),
        _ => panic!("expected sheet body"),
    };
    assert_ne!(sheet_stock, "komatex-foam-board");
    let sheet_drivers: std::collections::BTreeSet<_> = all
        .iter()
        .filter(|c| matches!(c.body, Some(Body::Sheet { .. })))
        .filter_map(|c| c.driver_id.as_deref())
        .collect();
    assert_eq!(sheet_drivers, ["test-woofer-a"].into());
    let result = optimize_combo(
        &design,
        &OptimizeRequest {
            seed: 5,
            combo: sheet.index,
            evaluations: 6,
        },
        &registry,
        &mut |_, _| true,
    )
    .unwrap();
    assert!(
        result
            .notes
            .iter()
            .any(|n| n.contains("Build method set to")),
        "{:?}",
        result.notes
    );
    assert!(!result.best.is_empty(), "expected at least one candidate");
    let best = &result.best[0];
    assert_eq!(best.design.speaker.enclosure.outer.kind, "sheet_box");
    assert!(
        best.variables
            .iter()
            .any(|(name, _)| name == "height / width"),
        "{:?}",
        best.variables
    );
    assert!(
        best.variables
            .iter()
            .any(|(name, _)| name == "depth / width"),
        "{:?}",
        best.variables
    );
    // Multi-way keeps the user's build method (no stock expansion).
    let two_way = combos(&example("two-way-vented"));
    assert_eq!(two_way.len(), 4);
    assert!(two_way
        .iter()
        .all(|c| matches!(c.body, Some(Body::Printed { .. }))));
}

#[test]
fn the_optimizer_repeats_from_its_seed_and_improves_on_the_start() {
    let registry = builtin();
    let design = example("two-way-vented");
    let all = combos(&design);
    // Multi-way: sealed/vented × LR4/LR2 on the current build only.
    assert_eq!(all.len(), 4);
    let vented = all
        .iter()
        .find(|c| {
            c.vented && c.crossover_order == Some(4) && matches!(c.body, Some(Body::Printed { .. }))
        })
        .unwrap();
    let request = OptimizeRequest {
        seed: 7,
        combo: vented.index,
        evaluations: 10,
    };
    let first = optimize_combo(&design, &request, &registry, &mut |_, _| true).unwrap();
    let again = optimize_combo(&design, &request, &registry, &mut |_, _| true).unwrap();
    assert_eq!(first.evaluations, again.evaluations);
    let summaries = |r: &seatlock_sim::optimize::ComboResult| {
        r.best.iter().map(|c| c.summary.clone()).collect::<Vec<_>>()
    };
    assert_eq!(summaries(&first), summaries(&again));
    let (start, _) = score(&design, &registry);
    let best = &first.best[0].score;
    assert!(
        best.shortfall < start.shortfall,
        "best {} start {}",
        best.shortfall,
        start.shortfall
    );
    // Every result is a complete design that simulates.
    let (rescored, _) = score_as_is(&first.best[0].design, &registry);
    assert!(rescored.problems.is_empty());
    // Canceling stops the search.
    let mut calls = 0;
    let canceled = optimize_combo(&design, &request, &registry, &mut |_, _| {
        calls += 1;
        calls < 2
    })
    .unwrap();
    assert!(canceled.canceled && canceled.evaluations < 10);
}
