// Rust guideline compliant 2026-06-22

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
    pub decode_errors: u32,
    pub zero_channel_packets: u32,
    pub truncated_frames: u32,
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

fn analyze_from_pcm_impl(
    sample_rate: u32,
    samples: *const f32,
    samples_len: usize,
    options_json: *const c_char,
) -> Result<MoodbarNativeAnalysisSummary, FfiError> {
    if samples.is_null() {
        return Err(FfiError::InvalidArgument(
            "samples pointer must not be null".to_string(),
        ));
    }

    let pcm = unsafe {
        // SAFETY: caller guarantees samples pointer is valid and contains samples_len elements.
        std::slice::from_raw_parts(samples, samples_len)
    };

    let options = parse_generate_options(options_json)?;
    let analysis = moodbar_analysis::analyze_pcm_mono(sample_rate, pcm, &options);
    store_analysis(analysis)
}

#[no_mangle]
pub extern "C" fn moodbar_native_analysis_from_pcm(
    sample_rate: u32,
    samples: *const f32,
    samples_len: usize,
    options_json: *const c_char,
    out_summary: *mut MoodbarNativeAnalysisSummary,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let summary = analyze_from_pcm_impl(sample_rate, samples, samples_len, options_json)?;
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
pub extern "C" fn moodbar_native_get_colors(
    handle: u64,
    out_colors: *mut MoodbarNativeBuffer,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let bytes = with_analysis(handle, |analysis| {
            let colors = analysis.colors();
            let mut out = Vec::<u8>::with_capacity(colors.len() * 3);
            for color in colors {
                out.push(color[0]);
                out.push(color[1]);
                out.push(color[2]);
            }
            Ok(out)
        })?;
        write_buffer(out_colors, bytes)
    })
}

#[no_mangle]
pub extern "C" fn moodbar_native_get_frames(
    handle: u64,
    out_frames: *mut MoodbarNativeBuffer,
) -> MoodbarNativeStatus {
    ffi_guard(|| {
        let bytes = with_analysis(handle, |analysis| {
            let mut out =
                Vec::<u8>::with_capacity(analysis.frames.len() * std::mem::size_of::<f64>());
            for &val in &analysis.frames {
                out.extend_from_slice(&val.to_ne_bytes());
            }
            Ok(out)
        })?;
        write_buffer(out_frames, bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_analysis_from_pcm() {
        let sample_rate = 44100;
        let samples = vec![0.0f32; 8000]; // 8k silence samples
        let mut summary = MoodbarNativeAnalysisSummary {
            handle: 0,
            frame_count: 0,
            channel_count: 0,
            decode_errors: 0,
            zero_channel_packets: 0,
            truncated_frames: 0,
        };

        let status = moodbar_native_analysis_from_pcm(
            sample_rate,
            samples.as_ptr(),
            samples.len(),
            ptr::null(),
            &mut summary,
        );

        assert_eq!(status as i32, MoodbarNativeStatus::Ok as i32);
        assert!(summary.handle > 0);
        assert!(summary.frame_count > 0);
        assert_eq!(summary.channel_count, 3); // Default has 3 channels

        // Render SVG to test it works
        let mut svg_buf = MoodbarNativeBuffer::empty();
        let render_status = moodbar_native_render_svg(summary.handle, ptr::null(), &mut svg_buf);
        assert_eq!(render_status as i32, MoodbarNativeStatus::Ok as i32);
        assert!(svg_buf.len > 0);

        unsafe {
            moodbar_native_buffer_free(&mut svg_buf);
        }

        // Dispose
        let dispose_status = moodbar_native_analysis_dispose(summary.handle);
        assert_eq!(dispose_status as i32, MoodbarNativeStatus::Ok as i32);
    }

    #[test]
    fn test_get_colors_and_frames() {
        let sample_rate = 44100;
        let samples = vec![0.1f32; 8000]; // some non-silent samples
        let mut summary = MoodbarNativeAnalysisSummary {
            handle: 0,
            frame_count: 0,
            channel_count: 0,
            decode_errors: 0,
            zero_channel_packets: 0,
            truncated_frames: 0,
        };

        let status = moodbar_native_analysis_from_pcm(
            sample_rate,
            samples.as_ptr(),
            samples.len(),
            ptr::null(),
            &mut summary,
        );

        assert_eq!(status as i32, MoodbarNativeStatus::Ok as i32);
        assert!(summary.handle > 0);
        assert!(summary.frame_count > 0);
        assert_eq!(summary.channel_count, 3);
        assert_eq!(summary.decode_errors, 0);
        assert_eq!(summary.zero_channel_packets, 0);
        assert_eq!(summary.truncated_frames, 0);

        // Retrieve the analyzed struct for reference assertion
        let mut expected_colors = Vec::new();
        let mut expected_flat_frames = Vec::new();
        let ref_status = with_analysis(summary.handle, |analysis| {
            expected_colors = analysis.colors().to_vec();
            expected_flat_frames = analysis.frames.clone();
            Ok(())
        });
        assert!(ref_status.is_ok());

        // Get colors
        let mut colors_buf = MoodbarNativeBuffer::empty();
        let colors_status = moodbar_native_get_colors(summary.handle, &mut colors_buf);
        assert_eq!(colors_status as i32, MoodbarNativeStatus::Ok as i32);
        assert_eq!(colors_buf.len, expected_colors.len() * 3);
        assert!(!colors_buf.ptr.is_null());

        let colors_slice = unsafe { std::slice::from_raw_parts(colors_buf.ptr, colors_buf.len) };
        for (i, &color) in expected_colors.iter().enumerate() {
            assert_eq!(colors_slice[i * 3], color[0]);
            assert_eq!(colors_slice[i * 3 + 1], color[1]);
            assert_eq!(colors_slice[i * 3 + 2], color[2]);
        }

        unsafe {
            moodbar_native_buffer_free(&mut colors_buf);
        }

        // Get frames
        let mut frames_buf = MoodbarNativeBuffer::empty();
        let frames_status = moodbar_native_get_frames(summary.handle, &mut frames_buf);
        assert_eq!(frames_status as i32, MoodbarNativeStatus::Ok as i32);
        assert_eq!(
            frames_buf.len,
            expected_flat_frames.len() * std::mem::size_of::<f64>()
        );
        assert!(!frames_buf.ptr.is_null());

        let frames_slice = unsafe {
            std::slice::from_raw_parts(frames_buf.ptr as *const f64, expected_flat_frames.len())
        };
        for (i, &expected_val) in expected_flat_frames.iter().enumerate() {
            assert!((frames_slice[i] - expected_val).abs() < 1e-12);
        }

        unsafe {
            moodbar_native_buffer_free(&mut frames_buf);
        }

        // Dispose
        let dispose_status = moodbar_native_analysis_dispose(summary.handle);
        assert_eq!(dispose_status as i32, MoodbarNativeStatus::Ok as i32);
    }
}
