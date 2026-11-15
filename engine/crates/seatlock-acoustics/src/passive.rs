//! Passive crossover networks (README section 7): R, L, and C between one amplifier and the
//! drivers, solved at each frequency with each driver's own impedance as the load.
//!
//! The amplifier is a voltage source of 1 V RMS between `source[0]` and `source[1]` (ground).
//! Node voltages and the complex voltage across each driver follow from modified nodal
//! analysis with the amp node fixed.

use num_complex::Complex64;
use serde_json::Value;
use std::collections::HashMap;
use std::f64::consts::PI;

/// One two-terminal part of a passive network.
#[derive(Clone, Debug)]
pub enum PassiveElement {
    R {
        a: String,
        b: String,
        ohm: f64,
    },
    L {
        a: String,
        b: String,
        henry: f64,
    },
    C {
        a: String,
        b: String,
        farad: f64,
    },
    /// Load: the radiator at this index, between nodes `a` and `b`.
    Driver {
        a: String,
        b: String,
        radiator: usize,
    },
}

/// A passive crossover: amplifier nodes and the netlist.
#[derive(Clone, Debug)]
pub struct PassiveNetwork {
    /// `[plus, minus]`; minus is ground (0 V).
    pub source: [String; 2],
    pub elements: Vec<PassiveElement>,
}

/// Result at one frequency: voltage across each driver (indexed by radiator), and the
/// impedance the amplifier sees.
#[derive(Clone, Debug)]
pub struct PassiveSample {
    /// `V_driver / V_amp` for each radiator index that appears as a driver load; missing
    /// radiators are absent from the map.
    pub voltage: HashMap<usize, Complex64>,
    pub amp_impedance_ohm: Complex64,
}

impl PassiveNetwork {
    pub fn from_value(raw: &Value) -> Result<Self, String> {
        let source = raw
            .get("source")
            .and_then(Value::as_array)
            .ok_or("passive crossover needs source: [plus, minus]")?;
        if source.len() != 2 {
            return Err("passive source must name the amplifier plus and ground nodes".into());
        }
        let plus = source[0]
            .as_str()
            .ok_or("passive source plus node must be a name")?
            .to_string();
        let minus = source[1]
            .as_str()
            .ok_or("passive source ground node must be a name")?
            .to_string();
        if plus == minus {
            return Err("passive source plus and ground must differ".into());
        }
        let list = raw
            .get("elements")
            .and_then(Value::as_array)
            .ok_or("passive crossover needs an elements list")?;
        if list.is_empty() {
            return Err("passive crossover has no elements".into());
        }
        let mut elements = Vec::with_capacity(list.len());
        for (index, item) in list.iter().enumerate() {
            elements.push(parse_element(item, index)?);
        }
        let drivers: Vec<usize> = elements
            .iter()
            .filter_map(|e| match e {
                PassiveElement::Driver { radiator, .. } => Some(*radiator),
                _ => None,
            })
            .collect();
        if drivers.is_empty() {
            return Err("passive crossover has no driver loads".into());
        }
        let mut seen = std::collections::HashSet::new();
        for id in &drivers {
            if !seen.insert(*id) {
                return Err(format!(
                    "passive crossover lists radiator {id} as a load more than once"
                ));
            }
        }
        Ok(PassiveNetwork {
            source: [plus, minus],
            elements,
        })
    }

    /// Voltage across each driver and the amplifier's load, at one frequency.
    /// `driver_z[i]` is the terminal impedance of radiator `i` (missing entries refuse).
    pub fn solve(&self, freq_hz: f64, driver_z: &[Complex64]) -> Result<PassiveSample, String> {
        let omega = 2.0 * PI * freq_hz.max(1e-9);
        let gnd = &self.source[1];
        let amp = &self.source[0];
        // Unknowns: every node except ground and the fixed amp node.
        let mut names: Vec<String> = Vec::new();
        let mut push = |name: &str| {
            if name != gnd && name != amp && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        };
        for element in &self.elements {
            let (a, b) = element_nodes(element);
            push(a);
            push(b);
        }
        let index_of = |name: &str| -> Option<usize> { names.iter().position(|n| n == name) };
        let n = names.len();
        let zero = Complex64::new(0.0, 0.0);
        let mut matrix = vec![vec![zero; n]; n];
        let mut rhs = vec![zero; n];

        // Stamp Y between a and b. Amp node is fixed at 1 V; ground at 0 V.
        for element in &self.elements {
            let (a, b) = element_nodes(element);
            if a == b {
                continue;
            }
            let y = element_y(element, omega, driver_z)?;
            match (a == gnd || a == amp, b == gnd || b == amp) {
                (false, false) => {
                    let i = index_of(a).unwrap();
                    let j = index_of(b).unwrap();
                    matrix[i][i] += y;
                    matrix[j][j] += y;
                    matrix[i][j] -= y;
                    matrix[j][i] -= y;
                }
                (true, true) => {}
                (false, true) => {
                    let i = index_of(a).unwrap();
                    matrix[i][i] += y;
                    if b == amp {
                        // KCL at a: y (Va - 1) + … = 0 → y Va on left, +y on RHS.
                        rhs[i] += y;
                    }
                }
                (true, false) => {
                    let j = index_of(b).unwrap();
                    matrix[j][j] += y;
                    if a == amp {
                        rhs[j] += y;
                    }
                }
            }
        }

        let voltages = if n == 0 {
            Vec::new()
        } else {
            let det_probe = matrix
                .iter()
                .map(|row| row.iter().map(|c| c.norm()).sum::<f64>())
                .sum::<f64>();
            if det_probe < 1e-30 {
                return Err("passive network is open or disconnected".into());
            }
            solve_system(matrix, rhs)
        };

        let v_of = |name: &str| -> Complex64 {
            if name == gnd {
                zero
            } else if name == amp {
                Complex64::new(1.0, 0.0)
            } else {
                index_of(name).map(|i| voltages[i]).unwrap_or(zero)
            }
        };

        // Amp current: sum Y (V_amp - V_other) over every element touching amp.
        let mut i_amp = zero;
        for element in &self.elements {
            let (a, b) = element_nodes(element);
            let y = element_y(element, omega, driver_z)?;
            if a == amp {
                i_amp += y * (Complex64::new(1.0, 0.0) - v_of(b));
            } else if b == amp {
                i_amp += y * (Complex64::new(1.0, 0.0) - v_of(a));
            }
        }

        let mut voltage = HashMap::new();
        for element in &self.elements {
            if let PassiveElement::Driver { a, b, radiator } = element {
                voltage.insert(*radiator, v_of(a) - v_of(b));
            }
        }
        let amp_impedance_ohm = if i_amp.norm() < 1e-18 {
            Complex64::new(f64::INFINITY, 0.0)
        } else {
            Complex64::new(1.0, 0.0) / i_amp
        };
        Ok(PassiveSample {
            voltage,
            amp_impedance_ohm,
        })
    }
}

fn element_nodes(element: &PassiveElement) -> (&str, &str) {
    match element {
        PassiveElement::R { a, b, .. }
        | PassiveElement::L { a, b, .. }
        | PassiveElement::C { a, b, .. }
        | PassiveElement::Driver { a, b, .. } => (a, b),
    }
}

fn element_y(
    element: &PassiveElement,
    omega: f64,
    driver_z: &[Complex64],
) -> Result<Complex64, String> {
    Ok(match element {
        PassiveElement::R { ohm, .. } => Complex64::new(1.0 / ohm, 0.0),
        PassiveElement::L { henry, .. } => Complex64::new(0.0, -1.0 / (omega * henry)),
        PassiveElement::C { farad, .. } => Complex64::new(0.0, omega * farad),
        PassiveElement::Driver { radiator, .. } => {
            let z = driver_z.get(*radiator).copied().unwrap_or_default();
            1.0 / z
        }
    })
}

fn parse_element(raw: &Value, index: usize) -> Result<PassiveElement, String> {
    let kind = raw
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("passive element {} has no kind", index + 1))?;
    let a = raw
        .get("a")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("passive element {} needs node a", index + 1))?
        .to_string();
    let b = raw
        .get("b")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("passive element {} needs node b", index + 1))?
        .to_string();
    match kind {
        "R" | "resistor" => {
            let ohm = number(raw, &["ohm", "ohms", "r"])
                .ok_or_else(|| format!("resistor {} needs ohm", index + 1))?;
            Ok(PassiveElement::R { a, b, ohm })
        }
        "L" | "inductor" => {
            let henry = number(raw, &["henry", "h", "l"])
                .ok_or_else(|| format!("inductor {} needs henry", index + 1))?;
            Ok(PassiveElement::L { a, b, henry })
        }
        "C" | "capacitor" => {
            let farad = number(raw, &["farad", "f", "c"])
                .ok_or_else(|| format!("capacitor {} needs farad", index + 1))?;
            Ok(PassiveElement::C { a, b, farad })
        }
        "driver" => {
            let radiator = raw
                .get("radiator")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("driver load {} needs radiator index", index + 1))?
                as usize;
            Ok(PassiveElement::Driver { a, b, radiator })
        }
        other => Err(format!(
            "passive element {} has unknown kind {other} (use R, L, C, or driver)",
            index + 1
        )),
    }
}

fn number(raw: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_f64))
        .filter(|v| v.is_finite() && *v > 0.0)
}

fn solve_system(mut a: Vec<Vec<Complex64>>, mut b: Vec<Complex64>) -> Vec<Complex64> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| a[i][col].norm().partial_cmp(&a[j][col].norm()).unwrap())
            .unwrap();
        a.swap(col, pivot);
        b.swap(col, pivot);
        let diag = a[col][col];
        if diag.norm() < 1e-30 {
            // Leave a zero row; result will be wrong — callers check connectivity.
            continue;
        }
        for row in col + 1..n {
            let factor = a[row][col] / diag;
            let pivot_row = a[col].clone();
            for (cell, value) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *cell -= factor * value;
            }
            let value = b[col];
            b[row] -= factor * value;
        }
    }
    let mut x = vec![Complex64::new(0.0, 0.0); n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for (coefficient, value) in a[row].iter().zip(&x).skip(row + 1) {
            sum -= coefficient * value;
        }
        if a[row][row].norm() < 1e-30 {
            x[row] = Complex64::new(0.0, 0.0);
        } else {
            x[row] = sum / a[row][row];
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_series_resistor_halves_the_voltage_into_an_equal_load() {
        let net = PassiveNetwork::from_value(&json!({
            "source": ["amp", "gnd"],
            "elements": [
                { "kind": "R", "a": "amp", "b": "w", "ohm": 8.0 },
                { "kind": "driver", "a": "w", "b": "gnd", "radiator": 0 }
            ]
        }))
        .unwrap();
        let z = [Complex64::new(8.0, 0.0)];
        let sample = net.solve(1000.0, &z).unwrap();
        let v = sample.voltage[&0];
        assert!((v.re - 0.5).abs() < 1e-9 && v.im.abs() < 1e-9, "{v}");
        assert!((sample.amp_impedance_ohm.re - 16.0).abs() < 1e-9);
    }

    #[test]
    fn a_series_inductor_is_a_first_order_lowpass_on_a_resistive_load() {
        // L = R / ωc → |V| = 1/sqrt(2) at fc.
        let r = 8.0;
        let fc = 1000.0;
        let l = r / (2.0 * PI * fc);
        let net = PassiveNetwork::from_value(&json!({
            "source": ["amp", "gnd"],
            "elements": [
                { "kind": "L", "a": "amp", "b": "w", "henry": l },
                { "kind": "driver", "a": "w", "b": "gnd", "radiator": 0 }
            ]
        }))
        .unwrap();
        let z = [Complex64::new(r, 0.0)];
        let at_fc = net.solve(fc, &z).unwrap().voltage[&0].norm();
        let low = net.solve(fc / 10.0, &z).unwrap().voltage[&0].norm();
        let high = net.solve(fc * 10.0, &z).unwrap().voltage[&0].norm();
        assert!(
            (at_fc - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "{at_fc}"
        );
        assert!((low - 1.0).abs() < 0.02, "{low}");
        assert!(high < 0.12, "{high}");
    }

    #[test]
    fn a_series_capacitor_is_a_first_order_highpass_on_a_resistive_load() {
        let r = 8.0;
        let fc = 2000.0;
        let c = 1.0 / (2.0 * PI * fc * r);
        let net = PassiveNetwork::from_value(&json!({
            "source": ["amp", "gnd"],
            "elements": [
                { "kind": "C", "a": "amp", "b": "t", "farad": c },
                { "kind": "driver", "a": "t", "b": "gnd", "radiator": 0 }
            ]
        }))
        .unwrap();
        let z = [Complex64::new(r, 0.0)];
        let at_fc = net.solve(fc, &z).unwrap().voltage[&0].norm();
        let low = net.solve(fc / 10.0, &z).unwrap().voltage[&0].norm();
        let high = net.solve(fc * 10.0, &z).unwrap().voltage[&0].norm();
        assert!(
            (at_fc - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "{at_fc}"
        );
        assert!(low < 0.12, "{low}");
        assert!((high - 1.0).abs() < 0.02, "{high}");
    }
}
