//! The generator (README section 11.4).

use seatlock_sim::builtin;
use seatlock_sim::generate::{families, generate_family, GenerateRequest};

fn example(name: &str) -> seatlock_core::DesignFile {
    let path = format!(
        "{}/../../../designs/examples/{name}.seatlock.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_family_proposes_cabinets_that_can_be_built_and_scored() {
    let base = example("sealed-test");
    for family in families() {
        let request = GenerateRequest {
            seed: 3,
            family: family.index,
            evaluations: 6,
        };
        let result = generate_family(&base, &request, &builtin(), &mut |_, _| true)
            .unwrap_or_else(|problem| panic!("{}: {problem}", family.label));
        assert_eq!(result.evaluations, 6, "{}", family.label);
        let top = &result.best[0];
        eprintln!(
            "{:44} bass {:5.0} Hz, even {:4.1} dB, clean {:4.0} dB, ${:6.2}, short {:5.2}, thrown out {:3} | {}",
            family.label,
            top.score.f6_hz,
            top.score.flatness_db,
            top.score.max_spl_db,
            top.score.cost_usd,
            top.score.shortfall,
            result.thrown_out,
            top.summary
        );
        // A real, priced driver in a cabinet the builder prints, with no errors.
        assert!(top.score.problems.is_empty(), "{:?}", top.score.problems);
        assert!(top.score.cost_usd > 0.0);
        let id = top.design.speaker.radiators[0].fields["driver_id"]
            .as_str()
            .unwrap();
        assert!(!id.starts_with("test-"));
        assert_eq!(top.design.speaker.enclosure.outer.kind, family.outer);
        assert_eq!(top.design.speaker.enclosure.mounts[0].surface, family.face);
        let report = seatlock_sim::geometry_report(&top.design).unwrap();
        assert!(report.can_export_print, "{:?}", report.warnings);
        assert_eq!(top.design.speaker.room.is_some(), family.face != "front");
        // Best first.
        for pair in result.best.windows(2) {
            let rank = |s: &seatlock_sim::compare::Score| {
                if s.meets_target {
                    s.cost_usd
                } else {
                    1e6 + 1e3 * s.shortfall
                }
            };
            assert!(rank(&pair[0].score) <= rank(&pair[1].score) + 1e-6);
        }
    }
}

#[test]
fn a_family_repeats_from_its_seed_and_stops_when_told() {
    let base = example("sealed-test");
    let request = GenerateRequest {
        seed: 7,
        family: 1,
        evaluations: 5,
    };
    let a = generate_family(&base, &request, &builtin(), &mut |_, _| true).unwrap();
    let b = generate_family(&base, &request, &builtin(), &mut |_, _| true).unwrap();
    assert_eq!(a.best[0].summary, b.best[0].summary);
    assert_eq!(a.best[0].score.shortfall, b.best[0].score.shortfall);
    let stopped = generate_family(&base, &request, &builtin(), &mut |done, _| done < 2).unwrap();
    assert!(stopped.canceled);
    assert_eq!(stopped.evaluations, 2);
}

#[test]
fn searching_longer_does_not_make_the_best_worse() {
    let base = example("sealed-test");
    let short = GenerateRequest {
        seed: 1,
        family: 1,
        evaluations: 8,
    };
    let long = GenerateRequest {
        seed: 1,
        family: 1,
        evaluations: 30,
    };
    let a = generate_family(&base, &short, &builtin(), &mut |_, _| true).unwrap();
    let b = generate_family(&base, &long, &builtin(), &mut |_, _| true).unwrap();
    eprintln!(
        "8: {:.3}  30: {:.3}",
        a.best[0].score.shortfall, b.best[0].score.shortfall
    );
    assert!(b.best[0].score.shortfall <= a.best[0].score.shortfall + 1e-9);
}
