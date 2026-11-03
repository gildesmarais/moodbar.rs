use crate::errors::ffi_guard;
use crate::{MoodbarNativeAnalysisSummary, MoodbarNativeBuffer, MoodbarNativeStatus};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jlong, jstring};
use jni::JNIEnv;
use serde_json::{json, Value};
use std::ffi::CString;

/// A JNI body yields the success payload or a ready-made `{"ok": false, ...}` envelope.
type Envelope = Result<Value, Value>;

/// Runs `body` under `ffi_guard`, so a panic becomes the Internal-status error envelope
/// carrying the guard's last-error message, then hands the envelope to the JVM.
fn guarded(env: &mut JNIEnv<'_>, body: impl FnOnce(&mut JNIEnv<'_>) -> Envelope) -> jstring {
    let mut envelope = None;
    let status = ffi_guard(|| {
        envelope = Some(body(env).unwrap_or_else(|error| error));
        Ok(())
    });
    let text = envelope
        .unwrap_or_else(|| last_error_json(status))
        .to_string();
    match env.new_string(&text) {
        Ok(s) => s.into_raw(),
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/OutOfMemoryError",
                format!("moodbar: failed to allocate JNI response string: {e}"),
            );
            std::ptr::null_mut()
        }
    }
}

fn invalid(error: impl std::fmt::Display) -> Value {
    json!({"ok": false, "status": MoodbarNativeStatus::InvalidArgument as i32, "error": error.to_string()})
}

fn c_string(env: &mut JNIEnv<'_>, value: &JString<'_>, name: &str) -> Result<CString, Value> {
    let text: String = env.get_string(value).map_err(invalid)?.into();
    CString::new(text).map_err(|_| invalid(format!("{name} contains NUL byte")))
}

fn take_buffer(mut buffer: MoodbarNativeBuffer) -> Vec<u8> {
    let bytes = if buffer.ptr.is_null() || buffer.len == 0 {
        Vec::new()
    } else {
        // SAFETY: a non-null buffer returned by this library holds `len` initialized bytes.
        unsafe { std::slice::from_raw_parts(buffer.ptr, buffer.len) }.to_vec()
    };
    // SAFETY: `buffer` came from this library and is freed exactly once here.
    unsafe { crate::moodbar_native_buffer_free(&mut buffer) };
    bytes
}

fn last_error_json(status: MoodbarNativeStatus) -> Value {
    let mut buffer = MoodbarNativeBuffer::empty();
    let _ = crate::moodbar_native_last_error(&mut buffer);
    let error = String::from_utf8_lossy(&take_buffer(buffer)).into_owned();
    json!({"ok": false, "status": status as i32, "error": error})
}

fn check(status: MoodbarNativeStatus) -> Result<(), Value> {
    match status {
        MoodbarNativeStatus::Ok => Ok(()),
        failed => Err(last_error_json(failed)),
    }
}

fn analyze(run: impl FnOnce(&mut MoodbarNativeAnalysisSummary) -> MoodbarNativeStatus) -> Envelope {
    let mut summary = MoodbarNativeAnalysisSummary {
        handle: 0,
        frame_count: 0,
        channel_count: 0,
        decode_errors: 0,
        zero_channel_packets: 0,
        truncated_frames: 0,
    };
    check(run(&mut summary))?;
    Ok(json!({
        "ok": true,
        "handle": summary.handle,
        "frameCount": summary.frame_count,
        "channelCount": summary.channel_count,
        "decodeErrors": summary.decode_errors,
        "zeroChannelPackets": summary.zero_channel_packets,
        "truncatedFrames": summary.truncated_frames,
    }))
}

fn render(
    export: extern "C" fn(
        u64,
        *const std::ffi::c_char,
        *mut MoodbarNativeBuffer,
    ) -> MoodbarNativeStatus,
    handle: jlong,
    options: &CString,
) -> Result<Vec<u8>, Value> {
    let mut out = MoodbarNativeBuffer::empty();
    check(export(handle as u64, options.as_ptr(), &mut out))?;
    Ok(take_buffer(out))
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeAnalyzeFromUri(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    uri: JString<'_>,
    options_json: JString<'_>,
) -> jstring {
    guarded(&mut env, |env| {
        let path = c_string(env, &uri, "uri")?;
        let opts = c_string(env, &options_json, "options JSON")?;
        analyze(|summary| {
            crate::moodbar_native_analysis_from_path(path.as_ptr(), opts.as_ptr(), summary)
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeAnalyzeFromBytes(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    bytes: JByteArray<'_>,
    extension: JObject<'_>,
    options_json: JString<'_>,
) -> jstring {
    guarded(&mut env, |env| {
        let bytes = env.convert_byte_array(&bytes).map_err(invalid)?;
        let ext = if extension.is_null() {
            None
        } else {
            Some(c_string(env, &JString::from(extension), "extension")?)
        };
        let ext_ptr = ext
            .as_ref()
            .filter(|ext| !ext.is_empty())
            .map_or(std::ptr::null(), |ext| ext.as_ptr());
        let opts = c_string(env, &options_json, "options JSON")?;
        analyze(|summary| {
            crate::moodbar_native_analysis_from_bytes(
                bytes.as_ptr(),
                bytes.len(),
                ext_ptr,
                opts.as_ptr(),
                summary,
            )
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeRenderSvg(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    options_json: JString<'_>,
) -> jstring {
    guarded(&mut env, |env| {
        let opts = c_string(env, &options_json, "options JSON")?;
        let svg = render(crate::moodbar_native_render_svg, handle, &opts)?;
        Ok(json!({"ok": true, "svg": String::from_utf8_lossy(&svg)}))
    })
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeRenderPng(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    options_json: JString<'_>,
) -> jstring {
    guarded(&mut env, |env| {
        let opts = c_string(env, &options_json, "options JSON")?;
        let png = render(crate::moodbar_native_render_png, handle, &opts)?;
        Ok(json!({"ok": true, "pngBase64": BASE64.encode(png)}))
    })
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeDisposeAnalysis(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jstring {
    guarded(&mut env, |_| {
        check(crate::moodbar_native_analysis_dispose(handle as u64))?;
        Ok(json!({"ok": true}))
    })
}
