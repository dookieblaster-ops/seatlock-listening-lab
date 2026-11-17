//! CHIEF vs plain CBIE near the first irregular frequency (CHK-BEM-5).
//!
//! Burton–Miller on constant discontinuous elements spoils the dipole check
//! (CHK-BEM-3); CHIEF keeps constant-element CBIE and kills the irregular peak.

use num_complex::Complex64;
use seatlock_bem::{icosphere, pulsating_sphere_pressure, BurtonMillerSystem, NeumannSystem};
use seatlock_core::C0;
use std::f64::consts::PI;

fn area_mean(mesh: &seatlock_bem::TriangleMesh, p: &[Complex64]) -> Complex64 {
    let mut sum = Complex64::new(0.0, 0.0);
    let mut w = 0.0;
    for (pi, a) in p.iter().zip(&mesh.areas) {
        sum += *pi * *a;
        w += *a;
    }
    sum / w
}

fn err_db(got: Complex64, expected: Complex64) -> f64 {
    20.0 * (got.norm() / expected.norm()).log10().abs()
}

#[test]
fn chk_bem_5_chief_holds_where_plain_cbie_breaks() {
    // Continuous first interior Dirichlet mode is ka = π. On a 320-face constant
    // icosphere the discrete irregular peak sits near ka ≈ 3.2.
    let a = 0.1;
    let mesh = icosphere(a, 2).expect("icosphere");
    assert_eq!(mesh.triangles.len(), 320);
    let ka = 3.2;
    let freq = ka * C0 / (2.0 * PI * a);
    let v = vec![Complex64::new(1.0, 0.0); mesh.triangles.len()];
    let expected = pulsating_sphere_pressure(ka, 1.0);

    let chief = NeumannSystem::assemble(&mesh, freq).expect("chief");
    let mean_chief = area_mean(&mesh, &chief.solve_velocity(&v).expect("chief solve"));
    let chief_db = err_db(mean_chief, expected);
    eprintln!(
        "CHIEF at ka={ka}: |p|={:.4} exact={:.4} err={chief_db:.3} dB phase={:.1}/{:.1}",
        mean_chief.norm(),
        expected.norm(),
        mean_chief.arg().to_degrees(),
        expected.arg().to_degrees()
    );
    assert!(
        chief_db < 0.5,
        "CHIEF at ka={ka}: {chief_db:.2} dB from closed form"
    );
    let phase_err = (mean_chief.arg() - expected.arg()).abs();
    assert!(
        phase_err < 10.0_f64.to_radians(),
        "CHIEF phase error {} deg",
        phase_err.to_degrees()
    );

    let cbie = NeumannSystem::assemble_cbie(&mesh, freq).expect("cbie assemble");
    let mean_cbie = area_mean(&mesh, &cbie.solve_velocity(&v).expect("cbie solve"));
    let cbie_db = err_db(mean_cbie, expected);
    eprintln!(
        "CBIE at ka={ka}: |p|={:.4} exact={:.4} err={cbie_db:.3} dB",
        mean_cbie.norm(),
        expected.norm()
    );
    assert!(
        cbie_db > 2.0,
        "plain CBIE should break near the discrete irregular peak; err was {cbie_db:.2} dB"
    );
    assert!(
        chief_db + 1.0 < cbie_db,
        "CHIEF should beat plain CBIE here (chief {chief_db:.2} vs cbie {cbie_db:.2})"
    );
}

#[test]
fn burton_miller_monopole_stays_usable_at_safe_ka() {
    // Kept as a monopole-only experiment: constant-element BM is not the radiation default.
    let a = 0.1;
    let mesh = icosphere(a, 2).unwrap();
    let v = vec![Complex64::new(1.0, 0.0); mesh.triangles.len()];
    let ka = 1.0;
    let freq = ka * C0 / (2.0 * PI * a);
    let expected = pulsating_sphere_pressure(ka, 1.0);
    let bm = BurtonMillerSystem::assemble(&mesh, freq).unwrap();
    let mean = area_mean(&mesh, &bm.solve_velocity(&v).unwrap());
    let e = err_db(mean, expected);
    eprintln!("BM monopole ka={ka}: err {e:.3} dB");
    assert!(e < 0.5, "ka={ka}: {e:.2} dB");
}
