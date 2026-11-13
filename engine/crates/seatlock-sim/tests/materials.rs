//! The novel-material catalog (README section 9.1): panel materials, layered panels, damping,
//! wall fills, and foaming filaments, and how a design uses them.

use seatlock_acoustics::laminate::{laminate, Layer};
use seatlock_core::catalog;
use seatlock_core::issue::Severity;
use seatlock_sim::{builtin, design_cost, geometry_report, simulate, SimulateRequest};
use serde_json::{json, Value};

fn example(name: &str) -> Value {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn design(value: &Value) -> seatlock_core::DesignFile {
    serde_json::from_value(value.clone()).unwrap()
}

fn sourced(record: &Value) {
    let id = record["id"].as_str().unwrap_or("?");
    if record["status"] == "synthetic" {
        return;
    }
    assert_eq!(record["status"], "sourced", "{id}");
    let source = record
        .get("source")
        .or_else(|| record.get("data_source"))
        .unwrap();
    assert!(
        source["url"].as_str().unwrap().starts_with("https://"),
        "{id}"
    );
    assert!(source["read_on"].as_str().is_some(), "{id}");
    assert!(
        record["note"].as_str().unwrap().len() > 40,
        "{id} needs a note"
    );
}

#[test]
fn every_panel_material_makes_a_panel_alone_or_as_a_core_between_pla_skins() {
    let records = catalog::all("catalog/panels/");
    assert!(records.len() >= 11, "{}", records.len());
    for record in records {
        let id = record["id"].as_str().unwrap();
        sourced(&record);
        let alone = Layer::from_catalog(id, 0.004, false).unwrap();
        if record["core_only"] == true {
            assert!(
                laminate(&[alone]).is_err(),
                "{id} has no bending stiffness alone"
            );
            let skin = || Layer::from_catalog("pla-printed", 0.0004, false).unwrap();
            let core = Layer::from_catalog(id, 0.006, false).unwrap();
            let stack = laminate(&[skin(), core, skin()]).unwrap();
            assert!(stack.stiffness.d11 > 0.0 && stack.stiffness.shear_per_width.is_finite());
        } else {
            let stack = laminate(&[alone]).unwrap();
            assert!(
                stack.stiffness.d11 > 0.0 && stack.stiffness.d22 > 0.0,
                "{id}"
            );
        }
    }
}

#[test]
fn every_damping_fill_and_foaming_filament_record_is_usable() {
    let damping = catalog::all("catalog/damping/");
    assert!(damping.len() >= 3);
    for record in &damping {
        sourced(record);
        assert!(record["flow_resistivity_pa_s_m2"].as_f64().unwrap() > 0.0);
        let id = record["id"].as_str().unwrap();
        assert!(catalog::damping(id).is_some());
    }
    let fills = catalog::all("catalog/fills/");
    assert!(fills.len() >= 2);
    for record in &fills {
        sourced(record);
        assert!(record["density_kg_m3"].as_f64().unwrap() > 1000.0);
        assert!(record["buy_kg_per_m3"].as_f64().unwrap() > 1000.0);
    }
    for id in ["lw_pla", "pla_aero", "varioshore_tpu", "roamr_tpu_air_85a"] {
        let record = catalog::material(id).unwrap();
        assert_eq!(record["foaming"], true, "{id}");
        let density = record["density_kg_m3"].as_f64().unwrap();
        assert!(density > 300.0 && density < 1000.0, "{id}: {density}");
        assert!(record.get("data_source").is_some(), "{id}");
    }
}

#[test]
fn a_sandwich_panel_speaker_simulates_and_prices_each_layer() {
    // PLA skins on a ThermHex honeycomb core, on the back of the rear-panel example.
    let mut value = example("rear-panel");
    let panel = &mut value["speaker"]["radiators"][1]["panel"];
    panel.as_object_mut().unwrap().remove("material");
    panel["thickness_m"] = json!(0.0058);
    panel["layers"] = json!([
        { "material": "pla-printed", "thickness_m": 0.0004 },
        { "material": "thermhex-pp-honeycomb", "thickness_m": 0.005 },
        { "material": "pla-printed", "thickness_m": 0.0004 }
    ]);
    let result = simulate(
        &design(&value),
        &SimulateRequest {
            radiation: "baffle".into(),
            ..SimulateRequest::default()
        },
        &builtin(),
    );
    let errors: Vec<_> = result
        .issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    let label = result.radiators[1].fidelity.ignored.join(" ");
    assert!(label.contains("3 layers"), "{label}");
    assert!(label.contains("(loss factor) is not published"), "{label}");
    let cost = design_cost(&design(&value));
    let panel_lines: Vec<_> = cost
        .lines
        .iter()
        .filter(|l| l.item.starts_with("Panel, "))
        .collect();
    assert_eq!(panel_lines.len(), 3);
    // The skins: 2 speakers x 0.30 x 0.20 x 0.0004 m x 1240 kg/m3 at $13.49/kg.
    let skin = panel_lines[0];
    let kg = 2.0 * 0.30 * 0.20 * 0.0004 * 1240.0;
    assert!((skin.quantity - kg).abs() < 1e-12, "{}", skin.quantity);
    assert!((skin.total_usd.unwrap() - kg * 13.49).abs() < 1e-9);
}

#[test]
fn a_birch_plywood_panel_is_stiffer_along_its_face_grain() {
    let veneer = |turned| Layer::from_catalog("birch-veneer", 0.001, turned).unwrap();
    let stack = laminate(&[veneer(false), veneer(true), veneer(false)]).unwrap();
    assert!(stack.stiffness.d11 > 5.0 * stack.stiffness.d22);
}

#[test]
fn a_wall_build_prices_its_core_by_the_hand_calculation() {
    let mut value = example("sealed-test");
    let skin = 0.0016;
    let wall = value["speaker"]["enclosure"]["outer"]["wall_m"]
        .as_f64()
        .unwrap();
    let solid = geometry_report(&design(&value)).unwrap().solid_volume_m3;
    let core = solid * (1.0 - 2.0 * skin / wall);
    let speakers = if value["build"]["stereo_pair"] == true {
        2.0
    } else {
        1.0
    };

    value["build"]["wall"] =
        json!({ "kind": "filled", "skin_m": skin, "fill": "quikrete-play-sand" });
    let cost = design_cost(&design(&value));
    let sand = catalog::fill("quikrete-play-sand").unwrap();
    let fill_line = cost
        .lines
        .iter()
        .find(|l| l.item.starts_with("Wall fill"))
        .unwrap();
    let kg = core * sand["buy_kg_per_m3"].as_f64().unwrap() * speakers;
    assert!(
        (fill_line.quantity - kg).abs() < 1e-9,
        "{} against {kg}",
        fill_line.quantity
    );
    let per_kg = sand["price"]["per_kg"].as_f64().unwrap();
    assert!((fill_line.total_usd.unwrap() - kg * per_kg).abs() < 1e-9);
    assert!(fill_line.unconfirmed);
    let pla = cost.lines.iter().find(|l| l.item == "Printed PLA").unwrap();
    let pla_kg = (solid - core) * 1240.0 * speakers;
    assert!(
        (pla.quantity - pla_kg).abs() < 1e-9,
        "{} against {pla_kg}",
        pla.quantity
    );
    assert!((cost.print_mass_kg - pla_kg).abs() < 1e-9);

    value["build"]["wall"] = json!({ "kind": "two_material", "skin_m": skin, "core": "lw_pla" });
    let cost = design_cost(&design(&value));
    let core_line = cost
        .lines
        .iter()
        .find(|l| l.item.starts_with("Printed wall core"))
        .unwrap();
    let lw_kg = core * 540.0 * speakers;
    assert!((core_line.quantity - lw_kg).abs() < 1e-9);
    assert!((core_line.total_usd.unwrap() - lw_kg * 40.74).abs() < 1e-9);
    assert!((cost.print_mass_kg - (pla_kg + lw_kg)).abs() < 1e-9);

    let result = simulate(&design(&value), &SimulateRequest::default(), &builtin());
    assert!(result
        .issues
        .iter()
        .any(|i| i.code == "wall_build_not_heard"));

    value["build"]["wall"] =
        json!({ "kind": "filled", "skin_m": 0.005, "fill": "quikrete-play-sand" });
    let cost = design_cost(&design(&value));
    assert!(cost.problems.iter().any(|p| p.contains("leave no room")));
}
