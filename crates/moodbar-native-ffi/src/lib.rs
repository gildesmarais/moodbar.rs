use std::ffi::c_char;
use std::path::Path;

use moodbar_analysis::{render_png, render_svg, MoodbarAnalysis};
use moodbar_decode::{analyze_bytes, analyze_path};

mod errors;
mod options;
mod registry;

pub use errors::MoodbarNativeStatus;
use errors::{ffi_guard, last_error_message, FfiError};
use options::{
    optional_c_string, parse_generate_options, parse_png_options, parse_svg_options,
    require_c_string,
};
use registry::{free_analysis, store_analysis, with_analysis};

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
    pub(crate) const fn empty() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        }
    }
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
    let svg = with_analysis(handle, |analysis: &MoodbarAnalysis| {
        Ok(render_svg(analysis, &options))
    })?;
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
mod android_jni;
