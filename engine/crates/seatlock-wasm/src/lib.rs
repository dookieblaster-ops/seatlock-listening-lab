//! WebAssembly worker entry: allocate, free, and JSON `call` for engine methods.

#![allow(clippy::missing_safety_doc)]

use seatlock_core::{design_check, CheckResult, ENGINE_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CallRequest {
    #[serde(default)]
    text: Option<String>,
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

fn dispatch(method: &str, request: &str) -> String {
    match method {
        "version" => serde_json::to_string(&VersionReply {
            version: ENGINE_VERSION,
        })
        .unwrap(),
        "design.check" => {
            let parsed: CallRequest = serde_json::from_str(request).unwrap_or(CallRequest {
                text: Some(request.to_string()),
            });
            let text = parsed
                .text
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| request.to_string());
            match design_check(&text) {
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
