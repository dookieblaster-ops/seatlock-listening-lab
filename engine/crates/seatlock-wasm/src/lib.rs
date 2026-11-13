//! WebAssembly worker entry: allocate, free, and JSON `call` for engine methods.

#![allow(clippy::missing_safety_doc)]

use seatlock_core::catalog;
use seatlock_core::{design_check, CheckResult, ENGINE_VERSION};
use seatlock_sim::{
    builtin, compare, correction_plan_with, geometry_report, listen_package, optimize,
    simulate_json, simulate_with_progress, SimulateRequest,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CallRequest {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    enclosure: Option<String>,
    #[serde(default)]
    radiation: Option<String>,
    #[serde(default)]
    space: Option<String>,
    #[serde(default)]
    voltage_v: Option<f64>,
    #[serde(default)]
    frequencies_hz: Option<Vec<f64>>,
    #[serde(default)]
    correction: Option<bool>,
    #[serde(default)]
    impulse_len: Option<usize>,
    #[serde(default)]
    sample_rate_hz: Option<f64>,
    #[serde(default)]
    fdtd_dx_m: Option<f64>,
    #[serde(default)]
    exterior_dx_m: Option<f64>,
}

#[derive(Serialize)]
struct VersionReply {
    version: &'static str,
}

#[derive(Serialize)]
struct DesignCheckReply {
    issues: Vec<seatlock_core::Issue>,
    canonical_json: String,
}

#[derive(Serialize)]
struct ErrorReply {
    error: String,
}

#[derive(Serialize)]
struct SimRequest {
    revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    enclosure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radiation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voltage_v: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequencies_hz: Option<Vec<f64>>,
}

fn read_utf8(ptr: *const u8, len: usize) -> Result<String, String> {
    if ptr.is_null() || len == 0 {
        return Ok(String::new());
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(slice)
        .map(|s| s.to_owned())
        .map_err(|e| format!("Invalid UTF-8: {e}"))
}

fn write_reply(json: String, out_len: *mut usize) -> *mut u8 {
    let bytes = json.into_bytes();
    let len = bytes.len();
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        unsafe { *out_len = 0 };
        return std::ptr::null_mut();
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        *out_len = len;
    }
    ptr
}

fn parse_request(request: &str) -> CallRequest {
    serde_json::from_str(request).unwrap_or(CallRequest {
        text: Some(request.to_string()),
        revision: 0,
        enclosure: None,
        radiation: None,
        space: None,
        voltage_v: None,
        frequencies_hz: None,
        correction: None,
        impulse_len: None,
        sample_rate_hz: None,
        fdtd_dx_m: None,
        exterior_dx_m: None,
    })
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    /// The page's progress hook: returns 0 to stop the run.
    fn seatlock_progress(steps: u32, max_steps: u32) -> u32;
}

fn report_progress(steps: usize, max_steps: usize) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { seatlock_progress(steps as u32, max_steps as u32) != 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (steps, max_steps);
        true
    }
}

/// Cost and quality comparisons and the optimizer (README sections 9.2, 6.2, 11.3). The
/// request is `{ "text": design, ...method fields }`.
fn compare_call(method: &str, request: &str) -> String {
    let error = |error: String| serde_json::to_string(&ErrorReply { error }).unwrap();
    let value: serde_json::Value = match serde_json::from_str(request) {
        Ok(value) => value,
        Err(err) => return error(err.to_string()),
    };
    let text = value.get("text").and_then(|t| t.as_str()).unwrap_or("");
    let design: seatlock_core::DesignFile = match serde_json::from_str(text) {
        Ok(design) => design,
        Err(err) => return error(err.to_string()),
    };
    let registry = builtin();
    let reply = match method {
        "compare.reference" => {
            serde_json::to_value(compare::cheap_against_reference(&design, &registry))
        }
        "compare.methods" => {
            let rows: Vec<serde_json::Value> = compare::build_methods(&design, &registry)
                .into_iter()
                .map(|row| match row {
                    Ok(variant) => serde_json::to_value(variant).unwrap_or_default(),
                    Err((label, problem)) => {
                        serde_json::json!({ "label": label, "error": problem })
                    }
                })
                .collect();
            Ok(serde_json::Value::Array(rows))
        }
        "optimize.combos" => serde_json::to_value(optimize::combos(&design)),
        "generate.families" => serde_json::to_value(seatlock_sim::generate::families()),
        "generate.family" => {
            let parsed: seatlock_sim::generate::GenerateRequest =
                match serde_json::from_value(value.clone()) {
                    Ok(parsed) => parsed,
                    Err(err) => return error(err.to_string()),
                };
            match seatlock_sim::generate::generate_family(
                &design,
                &parsed,
                &registry,
                &mut report_progress,
            ) {
                Ok(result) => serde_json::to_value(result),
                Err(problem) => return error(problem),
            }
        }
        "shape.inside" => {
            let parsed: seatlock_sim::shape::ShapeRequest =
                match serde_json::from_value(value.clone()) {
                    Ok(parsed) => parsed,
                    Err(err) => return error(err.to_string()),
                };
            match seatlock_sim::shape::shape_inside(
                &design,
                &parsed,
                &registry,
                &mut report_progress,
            ) {
                Ok(result) => {
                    let shaped = seatlock_sim::shape::apply(&design, &result);
                    Ok(serde_json::json!({ "result": result, "design": shaped }))
                }
                Err(problem) => return error(problem),
            }
        }
        _ => {
            let parsed: optimize::OptimizeRequest = match serde_json::from_value(value.clone()) {
                Ok(parsed) => parsed,
                Err(err) => return error(err.to_string()),
            };
            match optimize::optimize_combo(&design, &parsed, &registry, &mut report_progress) {
                Ok(result) => serde_json::to_value(result),
                Err(problem) => return error(problem),
            }
        }
    };
    match reply {
        Ok(value) => value.to_string(),
        Err(err) => error(err.to_string()),
    }
}

/// The full 3D model for the design as the builder makes it (README section 5.5).
fn full_3d(request: &str) -> String {
    let parsed = parse_request(request);
    let text = parsed.text.clone().unwrap_or_default();
    let design: seatlock_core::DesignFile = match serde_json::from_str(&text) {
        Ok(design) => design,
        Err(error) => {
            return serde_json::to_string(&ErrorReply {
                error: error.to_string(),
            })
            .unwrap()
        }
    };
    let defaults = SimulateRequest::default();
    let sim_request = SimulateRequest {
        enclosure: parsed.enclosure.unwrap_or_else(|| "fdtd".to_string()),
        radiation: parsed.radiation.unwrap_or(defaults.radiation),
        space: parsed.space.unwrap_or(defaults.space),
        voltage_v: parsed.voltage_v.unwrap_or(defaults.voltage_v),
        frequencies_hz: parsed.frequencies_hz,
        revision: parsed.revision,
        fdtd_dx_m: parsed.fdtd_dx_m,
        sound_power: false,
        listening_window: true,
        losses: true,
        exterior_dx_m: parsed.exterior_dx_m,
        fdtd_full_decay: false,
    };
    let result = simulate_with_progress(&design, &sim_request, &builtin(), &mut report_progress);
    serde_json::to_string(&result).unwrap_or_else(|error| {
        serde_json::to_string(&ErrorReply {
            error: error.to_string(),
        })
        .unwrap()
    })
}

fn dispatch(method: &str, request: &str) -> String {
    match method {
        "version" => serde_json::to_string(&VersionReply {
            version: ENGINE_VERSION,
        })
        .unwrap(),
        "design.check" => {
            let parsed = parse_request(request);
            let text = parsed
                .text
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| request.to_string());
            match design_check(&text, &builtin()) {
                Ok(CheckResult {
                    issues,
                    canonical_json,
                }) => serde_json::to_string(&DesignCheckReply {
                    issues,
                    canonical_json,
                })
                .unwrap(),
                Err(error) => serde_json::to_string(&ErrorReply { error }).unwrap(),
            }
        }
        "simulate" => {
            let parsed = parse_request(request);
            let text = parsed.text.clone().unwrap_or_default();
            let request_value = serde_json::to_string(&SimRequest {
                revision: parsed.revision,
                enclosure: parsed.enclosure,
                radiation: parsed.radiation,
                space: parsed.space,
                voltage_v: parsed.voltage_v,
                frequencies_hz: parsed.frequencies_hz,
            })
            .unwrap();
            match simulate_json(&text, &request_value, &builtin()) {
                Ok(value) => value.to_string(),
                Err(error) => serde_json::to_string(&ErrorReply { error }).unwrap(),
            }
        }
        "geometry" => {
            let parsed = parse_request(request);
            let text = parsed.text.unwrap_or_default();
            match serde_json::from_str(&text) {
                Ok(design) => match geometry_report(&design) {
                    Ok(report) => serde_json::to_string(&report).unwrap(),
                    Err(error) => serde_json::to_string(&ErrorReply { error }).unwrap(),
                },
                Err(error) => serde_json::to_string(&ErrorReply {
                    error: error.to_string(),
                })
                .unwrap(),
            }
        }
        "correct" => {
            let parsed = parse_request(request);
            let text = parsed.text.unwrap_or_default();
            match serde_json::from_str(&text) {
                Ok(design) => {
                    correction_plan_with(&design, &builtin(), "baffle", "half").to_string()
                }
                Err(error) => serde_json::to_string(&ErrorReply {
                    error: error.to_string(),
                })
                .unwrap(),
            }
        }
        "catalog.list" => {
            // The records of one catalog folder, or the print materials, as id and name.
            let parsed = parse_request(request);
            let folder = parsed.text.unwrap_or_default();
            let entries: Vec<serde_json::Value> = if folder.trim() == "materials" {
                catalog::embedded_file("catalog/materials.json")
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                    .and_then(|value| value.as_object().cloned())
                    .map(|map| {
                        map.into_iter()
                            .map(|(id, record)| {
                                serde_json::json!({
                                    "id": id,
                                    "name": record.get("name").cloned().unwrap_or(serde_json::Value::Null),
                                    "foaming": record.get("foaming").cloned().unwrap_or(serde_json::Value::Bool(false)),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                catalog::all(&format!("catalog/{}/", folder.trim()))
                    .into_iter()
                    .map(|record| {
                        serde_json::json!({
                            "id": record.get("id").cloned().unwrap_or_default(),
                            "name": record.get("name").cloned().unwrap_or_default(),
                            "core_only": record.get("core_only").cloned().unwrap_or(serde_json::Value::Bool(false)),
                            "flow_resistivity_pa_s_m2": record.get("flow_resistivity_pa_s_m2").cloned().unwrap_or_default(),
                            "status": record.get("status").cloned().unwrap_or_default(),
                        })
                    })
                    .collect()
            };
            serde_json::Value::Array(entries).to_string()
        }
        "catalog.driver" => {
            let parsed = parse_request(request);
            let id = parsed.text.unwrap_or_default();
            match catalog::driver(id.trim()) {
                Some(record) => record.to_string(),
                None => serde_json::to_string(&ErrorReply {
                    error: format!("unknown driver {id}"),
                })
                .unwrap(),
            }
        }
        "listen" => {
            let parsed = parse_request(request);
            let text = parsed.text.unwrap_or_default();
            match serde_json::from_str(&text) {
                Ok(design) => listen_package(
                    &design,
                    &builtin(),
                    parsed.sample_rate_hz.unwrap_or(0.0),
                    parsed.correction.unwrap_or(true),
                    parsed.impulse_len.unwrap_or(0),
                )
                .to_string(),
                Err(error) => serde_json::to_string(&ErrorReply {
                    error: error.to_string(),
                })
                .unwrap(),
            }
        }
        "simulate.full3d" => full_3d(request),
        "compare.reference" | "compare.methods" | "optimize.combos" | "optimize.combo"
        | "shape.inside" | "generate.families" | "generate.family" => compare_call(method, request),
        other => serde_json::to_string(&ErrorReply {
            error: format!("Unknown method: {other}"),
        })
        .unwrap(),
    }
}

#[no_mangle]
pub extern "C" fn seatlock_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let layout = match std::alloc::Layout::from_size_align(len, 1) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };
    unsafe { std::alloc::alloc(layout) }
}

#[no_mangle]
pub unsafe extern "C" fn seatlock_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    std::alloc::dealloc(ptr, layout);
}

/// Returns a pointer to a JSON reply; sets `out_len` to the byte length. Free with `seatlock_free`.
#[no_mangle]
pub unsafe extern "C" fn seatlock_call(
    method_ptr: *const u8,
    method_len: usize,
    req_ptr: *const u8,
    req_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    let method = match read_utf8(method_ptr, method_len) {
        Ok(m) => m,
        Err(error) => {
            return write_reply(
                serde_json::to_string(&ErrorReply { error }).unwrap(),
                out_len,
            )
        }
    };
    let request = match read_utf8(req_ptr, req_len) {
        Ok(r) => r,
        Err(error) => {
            return write_reply(
                serde_json::to_string(&ErrorReply { error }).unwrap(),
                out_len,
            )
        }
    };
    let reply = dispatch(&method, &request);
    write_reply(reply, out_len)
}
