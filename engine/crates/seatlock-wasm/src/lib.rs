//! WebAssembly worker entry: allocate, free, and JSON `call` for engine methods.

#![allow(clippy::missing_safety_doc)]

use seatlock_core::catalog;
use seatlock_core::{design_check, CheckResult, ENGINE_VERSION};
use seatlock_fdtd::{aperture_cells, run_grid};
use seatlock_sim::{builtin, correction_plan_with, geometry_report, listen_package, simulate_json};
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
    })
}

fn fdtd_run(request: &str) -> Result<seatlock_fdtd::ImpedanceCurve, String> {
    let value: serde_json::Value = serde_json::from_str(request).map_err(|err| err.to_string())?;
    let depth = value
        .get("depth_m")
        .and_then(|v| v.as_f64())
        .ok_or("missing depth_m")?;
    let width = value
        .get("width_m")
        .and_then(|v| v.as_f64())
        .ok_or("missing width_m")?;
    let height = value
        .get("height_m")
        .and_then(|v| v.as_f64())
        .ok_or("missing height_m")?;
    let dx = value.get("dx").and_then(|v| v.as_f64()).unwrap_or(0.01);
    let center_y = value
        .get("center_y_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(width * 0.5);
    let center_z = value
        .get("center_z_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(height * 0.5);
    let radius = value
        .get("radius_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.02);
    let nx = (depth / dx).round().max(1.0) as usize;
    let ny = (width / dx).round().max(1.0) as usize;
    let nz = (height / dx).round().max(1.0) as usize;
    let source = aperture_cells(ny, nz, dx, [center_y, center_z], radius);
    Ok(run_grid(nx, ny, nz, dx, &source, depth * width * height))
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
        "fdtd.run" => match fdtd_run(request) {
            Ok(curve) => serde_json::json!({
                "frequencies_hz": curve.frequencies_hz,
                "real": curve.real,
                "imag": curve.imag,
                "steps": curve.steps,
                "elapsed_ms": curve.elapsed_ms,
                "grid_volume_m3": curve.grid_volume_m3,
                "true_volume_m3": curve.true_volume_m3,
                "warning": curve.warning,
                "ignored": curve.ignored,
            })
            .to_string(),
            Err(error) => serde_json::to_string(&ErrorReply { error }).unwrap(),
        },
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
