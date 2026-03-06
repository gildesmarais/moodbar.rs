use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use moodbar_analysis::{
    render_png, render_svg, GenerateOptions, MoodbarAnalysis, MoodbarError, PngOptions, SvgOptions,
};
use moodbar_bindings_schema::{
    apply_generate_patch, apply_png_patch, apply_svg_patch, GenerateOptionsPatch, PngOptionsPatch,
    SvgOptionsPatch,
};
use moodbar_decode::{analyze_bytes, analyze_path, MoodbarDecodeError};
use once_cell::sync::Lazy;
use serde::Deserialize;
use thiserror::Error;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MoodbarNativeAnalysisSummary {
    pub handle: u64,
    pub frame_count: u32,
    pub channel_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MoodbarNativeBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl MoodbarNativeBuffer {
    const fn empty() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum MoodbarNativeStatus {
    Ok = 0,
    InvalidArgument = 1,
    NotFound = 2,
    Internal = 3,
}

impl MoodbarNativeStatus {
    fn from_error(err: &FfiError) -> Self {
        match err {
            FfiError::InvalidArgument(_) => Self::InvalidArgument,
            FfiError::NotFound(_) => Self::NotFound,
            FfiError::Decode(decode_err) => match decode_err {
                MoodbarDecodeError::NoAudioTrack
                | MoodbarDecodeError::EmptyAudio
                | MoodbarDecodeError::InvalidOptions(_) => Self::InvalidArgument,
                MoodbarDecodeError::Io(io_err) => {
                    if io_err.kind() == std::io::ErrorKind::NotFound {
                        Self::NotFound
                    } else {
                        Self::Internal
                    }
                }
                MoodbarDecodeError::Decode(_) => Self::Internal,
            },
            FfiError::Analysis(analysis_err) => match analysis_err {
                MoodbarError::InvalidOptions(_) => Self::InvalidArgument,
                MoodbarError::Image(_) => Self::Internal,
            },
            FfiError::Poisoned | FfiError::Panic | FfiError::Utf8 | FfiError::Json(_) => {
                Self::Internal
            }
        }
    }
}

#[derive(Debug, Error)]
enum FfiError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Decode(#[from] MoodbarDecodeError),
    #[error(transparent)]
    Analysis(#[from] MoodbarError),
    #[error("mutex poisoned")]
    Poisoned,
    #[error("panic across FFI boundary")]
    Panic,
    #[error("invalid UTF-8 in C string")]
    Utf8,
    #[error("invalid options JSON: {0}")]
    Json(String),
}

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static ANALYSIS_REGISTRY: Lazy<Mutex<HashMap<u64, MoodbarAnalysis>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn parse_generate_options(options_json: *const c_char) -> Result<GenerateOptions, FfiError> {
    let input: GenerateOptionsPatch = parse_optional_json(options_json)?;
    let mut options = GenerateOptions::default();
    apply_generate_patch(&mut options, input);
    Ok(options)
}

fn parse_svg_options(options_json: *const c_char) -> Result<SvgOptions, FfiError> {
    let input: SvgOptionsPatch = parse_optional_json(options_json)?;
    let mut options = SvgOptions::default();
    apply_svg_patch(&mut options, input).map_err(FfiError::InvalidArgument)?;
    Ok(options)
}

fn parse_png_options(options_json: *const c_char) -> Result<PngOptions, FfiError> {
    let input: PngOptionsPatch = parse_optional_json(options_json)?;
    let mut options = PngOptions::default();
    apply_png_patch(&mut options, input);
    Ok(options)
}

fn parse_optional_json<T>(json_cstr: *const c_char) -> Result<T, FfiError>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if json_cstr.is_null() {
        return Ok(T::default());
    }
    let raw = unsafe {
        // SAFETY: Caller provides a null-terminated C string pointer.
        CStr::from_ptr(json_cstr)
    };

    if raw.to_bytes().is_empty() {
        return Ok(T::default());
    }

    let text = raw.to_str().map_err(|_| FfiError::Utf8)?;
    serde_json::from_str(text).map_err(|e| FfiError::Json(e.to_string()))
}

fn require_c_string<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, FfiError> {
    if ptr.is_null() {
        return Err(FfiError::InvalidArgument(format!(
            "{name} pointer must not be null"
        )));
    }

    let raw = unsafe {
        // SAFETY: pointer validity is guaranteed by the caller contract.
        CStr::from_ptr(ptr)
    };
    raw.to_str().map_err(|_| FfiError::Utf8)
}

fn optional_c_string<'a>(ptr: *const c_char) -> Result<Option<&'a str>, FfiError> {
    if ptr.is_null() {
        return Ok(None);
    }

    let raw = unsafe {
        // SAFETY: pointer validity is guaranteed by the caller contract.
        CStr::from_ptr(ptr)
    };
    let value = raw.to_str().map_err(|_| FfiError::Utf8)?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn store_analysis(analysis: MoodbarAnalysis) -> Result<MoodbarNativeAnalysisSummary, FfiError> {
    let frame_count = analysis.frames.len() as u32;
    let channel_count = analysis.channel_count as u32;
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);

    let mut guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    guard.insert(handle, analysis);

    Ok(MoodbarNativeAnalysisSummary {
        handle,
        frame_count,
        channel_count,
    })
}

fn with_analysis<R>(
    handle: u64,
    f: impl FnOnce(&MoodbarAnalysis) -> Result<R, FfiError>,
) -> Result<R, FfiError> {
    let guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    let analysis = guard
        .get(&handle)
        .ok_or_else(|| FfiError::NotFound(format!("analysis handle {handle} not found")))?;
    f(analysis)
}

fn free_analysis(handle: u64) -> Result<(), FfiError> {
    let mut guard = ANALYSIS_REGISTRY.lock().map_err(|_| FfiError::Poisoned)?;
    if guard.remove(&handle).is_none() {
        return Err(FfiError::NotFound(format!(
            "analysis handle {handle} not found"
        )));
    }
    Ok(())
}

fn write_summary(
    out_summary: *mut MoodbarNativeAnalysisSummary,
    summary: MoodbarNativeAnalysisSummary,
) -> Result<(), FfiError> {
    if out_summary.is_null() {
        return Err(FfiError::InvalidArgument(
            "out_summary pointer must not be null".to_string(),
        ));
    }

    unsafe {
        // SAFETY: caller guarantees `out_summary` points to writable memory.
        *out_summary = summary;
    }

    Ok(())
}

fn write_buffer(out_buffer: *mut MoodbarNativeBuffer, bytes: Vec<u8>) -> Result<(), FfiError> {
    if out_buffer.is_null() {
        return Err(FfiError::InvalidArgument(
            "out_buffer pointer must not be null".to_string(),
        ));
    }

    let mut bytes = bytes;
    let buffer = MoodbarNativeBuffer {
        ptr: bytes.as_mut_ptr(),
        len: bytes.len(),
        cap: bytes.capacity(),
    };
    std::mem::forget(bytes);

    unsafe {
        // SAFETY: caller guarantees `out_buffer` points to writable memory.
        *out_buffer = buffer;
    }

    Ok(())
}

fn set_last_error(message: String) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = message;
    });
}

fn clear_last_error() {
    set_last_error(String::new());
}

fn last_error_message() -> String {
    LAST_ERROR.with(|cell| cell.borrow().clone())
}

fn ffi_status_from_result(result: Result<(), FfiError>) -> MoodbarNativeStatus {
    match result {
        Ok(()) => {
            clear_last_error();
            MoodbarNativeStatus::Ok
        }
        Err(err) => {
            set_last_error(err.to_string());
            MoodbarNativeStatus::from_error(&err)
        }
    }
}

fn ffi_guard(f: impl FnOnce() -> Result<(), FfiError>) -> MoodbarNativeStatus {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    match caught {
        Ok(result) => ffi_status_from_result(result),
        Err(_) => ffi_status_from_result(Err(FfiError::Panic)),
    }
}

fn analyze_from_path_impl(
    path: *const c_char,
    options_json: *const c_char,
) -> Result<MoodbarNativeAnalysisSummary, FfiError> {
    let path_str = require_c_string(path, "path")?;
    let options = parse_generate_options(options_json)?;
    let analysis = analyze_path(Path::new(path_str), &options)?;
    store_analysis(analysis)
}

fn analyze_from_bytes_impl(
    bytes: *const u8,
    bytes_len: usize,
    extension: *const c_char,
    options_json: *const c_char,
) -> Result<MoodbarNativeAnalysisSummary, FfiError> {
    if bytes.is_null() {
        return Err(FfiError::InvalidArgument(
            "bytes pointer must not be null".to_string(),
        ));
    }

    let encoded = unsafe {
        // SAFETY: pointer/len are provided by caller and valid for the duration of this call.
        std::slice::from_raw_parts(bytes, bytes_len)
    };

    let ext = optional_c_string(extension)?;
    let options = parse_generate_options(options_json)?;
    let analysis = analyze_bytes(encoded, ext, &options)?;
    store_analysis(analysis)
}

fn render_svg_impl(handle: u64, options_json: *const c_char) -> Result<Vec<u8>, FfiError> {
    let options = parse_svg_options(options_json)?;
    let svg = with_analysis(handle, |analysis| Ok(render_svg(analysis, &options)))?;
    Ok(svg.into_bytes())
}

fn render_png_impl(handle: u64, options_json: *const c_char) -> Result<Vec<u8>, FfiError> {
    let options = parse_png_options(options_json)?;
    with_analysis(handle, |analysis| {
        render_png(analysis, &options).map_err(FfiError::from)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_analysis_from_path(
    path: *const c_char,
    options_json: *const c_char,
    out_summary: *mut MoodbarNativeAnalysisSummary,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let summary = analyze_from_path_impl(path, options_json)?;
        write_summary(out_summary, summary)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_analysis_from_bytes(
    bytes: *const u8,
    bytes_len: usize,
    extension: *const c_char,
    options_json: *const c_char,
    out_summary: *mut MoodbarNativeAnalysisSummary,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let summary = analyze_from_bytes_impl(bytes, bytes_len, extension, options_json)?;
        write_summary(out_summary, summary)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_analysis_dispose(handle: u64) -> MoodbarNativeStatus {
    ffi_guard(|| free_analysis(handle))
}

#[no_mangle]
pub extern "C" fn moodbar_native_render_svg(
    handle: u64,
    options_json: *const c_char,
    out_svg_utf8: *mut MoodbarNativeBuffer,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let bytes = render_svg_impl(handle, options_json)?;
        write_buffer(out_svg_utf8, bytes)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_render_png(
    handle: u64,
    options_json: *const c_char,
    out_png: *mut MoodbarNativeBuffer,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let bytes = render_png_impl(handle, options_json)?;
        write_buffer(out_png, bytes)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_last_error(
    out_message_utf8: *mut MoodbarNativeBuffer,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let message = last_error_message();
        write_buffer(out_message_utf8, message.into_bytes())
    })
}

#[no_mangle]
/// Frees a heap buffer previously returned by this library.
///
/// # Safety
///
/// `buffer` must be a valid, writable pointer obtained from this library, and must not be
/// freed more than once.
pub unsafe extern "C" fn moodbar_native_buffer_free(buffer: *mut MoodbarNativeBuffer) {
    if buffer.is_null() {
        return;
    }

    // SAFETY: caller guarantees pointer is valid and originated from `write_buffer`.
    let buf = &mut *buffer;
    if !buf.ptr.is_null() && buf.cap > 0 {
        let _ = Vec::from_raw_parts(buf.ptr, buf.len, buf.cap);
    }
    *buf = MoodbarNativeBuffer::empty();
}

#[cfg(target_os = "android")]
mod android_jni {
    use super::MoodbarNativeBuffer;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use jni::objects::{JByteArray, JClass, JObject, JString};
    use jni::sys::{jlong, jstring};
    use jni::JNIEnv;
    use serde_json::json;
    use std::ffi::CString;

    fn to_rust_string(env: &mut JNIEnv<'_>, value: JString<'_>) -> Result<String, String> {
        env.get_string(&value)
            .map(|s| s.into())
            .map_err(|e| e.to_string())
    }

    fn optional_rust_string(
        env: &mut JNIEnv<'_>,
        value: JObject<'_>,
    ) -> Result<Option<String>, String> {
        if value.is_null() {
            return Ok(None);
        }
        let jstr = JString::from(value);
        let s = to_rust_string(env, jstr)?;
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    fn response(env: &mut JNIEnv<'_>, value: serde_json::Value) -> jstring {
        let text = value.to_string();
        match env.new_string(text) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }

    fn status_message(mut buffer: MoodbarNativeBuffer) -> String {
        let message = if buffer.ptr.is_null() || buffer.len == 0 {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) };
            String::from_utf8_lossy(bytes).to_string()
        };
        unsafe {
            super::moodbar_native_buffer_free(&mut buffer);
        }
        message
    }

    fn last_error_json() -> serde_json::Value {
        let mut buffer = MoodbarNativeBuffer::empty();
        let status = super::moodbar_native_last_error(&mut buffer);
        json!({
            "ok": false,
            "status": status as i32,
            "error": status_message(buffer),
        })
    }

    fn cstring_opt(value: Option<String>) -> Result<Option<CString>, serde_json::Value> {
        match value {
            None => Ok(None),
            Some(v) => CString::new(v).map(Some).map_err(
                |_| json!({"ok": false, "status": 1, "error": "string contains NUL byte"}),
            ),
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeAnalyzeFromUri(
        mut env: JNIEnv<'_>,
        _class: JClass<'_>,
        uri: JString<'_>,
        options_json: JString<'_>,
    ) -> jstring {
        let uri = match to_rust_string(&mut env, uri) {
            Ok(value) => value,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };
        let options = match to_rust_string(&mut env, options_json) {
            Ok(value) => value,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };

        let path = match CString::new(uri) {
            Ok(v) => v,
            Err(_) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": "uri contains NUL byte"}),
                )
            }
        };
        let opts = match CString::new(options) {
            Ok(v) => v,
            Err(_) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": "options JSON contains NUL byte"}),
                )
            }
        };

        let mut summary = super::MoodbarNativeAnalysisSummary {
            handle: 0,
            frame_count: 0,
            channel_count: 0,
        };
        let status =
            super::moodbar_native_analysis_from_path(path.as_ptr(), opts.as_ptr(), &mut summary);
        if (status as i32) != 0 {
            return response(&mut env, last_error_json());
        }

        response(
            &mut env,
            json!({
                "ok": true,
                "handle": summary.handle,
                "frameCount": summary.frame_count,
                "channelCount": summary.channel_count,
            }),
        )
    }

    #[no_mangle]
    pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeAnalyzeFromBytes(
        mut env: JNIEnv<'_>,
        _class: JClass<'_>,
        bytes: JByteArray<'_>,
        extension: JObject<'_>,
        options_json: JString<'_>,
    ) -> jstring {
        let bytes = match env.convert_byte_array(&bytes) {
            Ok(v) => v,
            Err(e) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": e.to_string()}),
                )
            }
        };

        let ext = match optional_rust_string(&mut env, extension) {
            Ok(v) => v,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };
        let ext_c = match cstring_opt(ext) {
            Ok(v) => v,
            Err(error) => return response(&mut env, error),
        };

        let options = match to_rust_string(&mut env, options_json) {
            Ok(value) => value,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };
        let opts = match CString::new(options) {
            Ok(v) => v,
            Err(_) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": "options JSON contains NUL byte"}),
                )
            }
        };

        let mut summary = super::MoodbarNativeAnalysisSummary {
            handle: 0,
            frame_count: 0,
            channel_count: 0,
        };
        let ext_ptr = ext_c
            .as_ref()
            .map(|value| value.as_ptr())
            .unwrap_or(std::ptr::null());
        let status = super::moodbar_native_analysis_from_bytes(
            bytes.as_ptr() as *const u8,
            bytes.len(),
            ext_ptr,
            opts.as_ptr(),
            &mut summary,
        );
        if (status as i32) != 0 {
            return response(&mut env, last_error_json());
        }

        response(
            &mut env,
            json!({
                "ok": true,
                "handle": summary.handle,
                "frameCount": summary.frame_count,
                "channelCount": summary.channel_count,
            }),
        )
    }

    #[no_mangle]
    pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeRenderSvg(
        mut env: JNIEnv<'_>,
        _class: JClass<'_>,
        handle: jlong,
        options_json: JString<'_>,
    ) -> jstring {
        let options = match to_rust_string(&mut env, options_json) {
            Ok(value) => value,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };
        let opts = match CString::new(options) {
            Ok(v) => v,
            Err(_) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": "options JSON contains NUL byte"}),
                )
            }
        };

        let mut out = MoodbarNativeBuffer::empty();
        let status = super::moodbar_native_render_svg(handle as u64, opts.as_ptr(), &mut out);
        if (status as i32) != 0 {
            return response(&mut env, last_error_json());
        }

        let svg = status_message(out);
        response(&mut env, json!({"ok": true, "svg": svg}))
    }

    #[no_mangle]
    pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeRenderPng(
        mut env: JNIEnv<'_>,
        _class: JClass<'_>,
        handle: jlong,
        options_json: JString<'_>,
    ) -> jstring {
        let options = match to_rust_string(&mut env, options_json) {
            Ok(value) => value,
            Err(error) => {
                return response(&mut env, json!({"ok": false, "status": 1, "error": error}))
            }
        };
        let opts = match CString::new(options) {
            Ok(v) => v,
            Err(_) => {
                return response(
                    &mut env,
                    json!({"ok": false, "status": 1, "error": "options JSON contains NUL byte"}),
                )
            }
        };

        let mut out = MoodbarNativeBuffer::empty();
        let status = super::moodbar_native_render_png(handle as u64, opts.as_ptr(), &mut out);
        if (status as i32) != 0 {
            return response(&mut env, last_error_json());
        }

        let encoded = if out.ptr.is_null() || out.len == 0 {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(out.ptr, out.len) };
            BASE64.encode(bytes)
        };
        unsafe {
            super::moodbar_native_buffer_free(&mut out);
        }
        response(&mut env, json!({"ok": true, "pngBase64": encoded}))
    }

    #[no_mangle]
    pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeDisposeAnalysis(
        mut env: JNIEnv<'_>,
        _class: JClass<'_>,
        handle: jlong,
    ) -> jstring {
        let status = super::moodbar_native_analysis_dispose(handle as u64);
        if (status as i32) != 0 {
            return response(&mut env, last_error_json());
        }
        response(&mut env, json!({"ok": true}))
    }
}
