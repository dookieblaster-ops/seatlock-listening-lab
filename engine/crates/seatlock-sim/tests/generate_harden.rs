//! Multi-source generator harden: stacked and folded-line families must build often enough
//! that a short search is not burned on throw-outs (README 2.6).

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
fn multi_source_families_throw_out_far_fewer_than_they_score() {
    let base = example("sealed-test");
    for label in [
        "Box, folded line, woofer and tweeter",
        "Box, ported, two woofers around a tweeter",
        "Box, ported, woofer and tweeter",
    ] {
        let family = families()
            .into_iter()
            .find(|f| f.label == label)
            .unwrap_or_else(|| panic!("missing family {label}"));
        let request = GenerateRequest {
            seed: 3,
            family: family.index,
            evaluations: 8,
        };
        let result = generate_family(&base, &request, &builtin(), &mut |_, _| true)
            .unwrap_or_else(|problem| panic!("{label}: {problem}"));
        assert_eq!(result.evaluations, 8, "{label}");
        // Before harden these threw out ~49–219 on six evaluations. Keep throw-outs under
        // a few times the scored count so the search spends most of its tries on real cabinets.
        assert!(
            result.thrown_out <= result.evaluations * 8,
            "{label}: thrown out {} for {} scored ({:?})",
            result.thrown_out,
            result.evaluations,
            result.notes
        );
        assert!(result.best[0].score.problems.is_empty());
    }
}
