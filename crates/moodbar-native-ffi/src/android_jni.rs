use crate::MoodbarNativeBuffer;
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
        crate::moodbar_native_buffer_free(&mut buffer);
    }
    message
}

fn last_error_json() -> serde_json::Value {
    let mut buffer = MoodbarNativeBuffer::empty();
    let status = crate::moodbar_native_last_error(&mut buffer);
    json!({
        "ok": false,
        "status": status as i32,
        "error": status_message(buffer),
    })
}

fn cstring_opt(value: Option<String>) -> Result<Option<CString>, serde_json::Value> {
    match value {
        None => Ok(None),
        Some(v) => CString::new(v)
            .map(Some)
            .map_err(|_| json!({"ok": false, "status": 1, "error": "string contains NUL byte"})),
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
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
    };
    let options = match to_rust_string(&mut env, options_json) {
        Ok(value) => value,
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
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

    let mut summary = crate::MoodbarNativeAnalysisSummary {
        handle: 0,
        frame_count: 0,
        channel_count: 0,
    };
    let status =
        crate::moodbar_native_analysis_from_path(path.as_ptr(), opts.as_ptr(), &mut summary);
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
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
    };
    let ext_c = match cstring_opt(ext) {
        Ok(v) => v,
        Err(error) => return response(&mut env, error),
    };

    let options = match to_rust_string(&mut env, options_json) {
        Ok(value) => value,
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
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

    let mut summary = crate::MoodbarNativeAnalysisSummary {
        handle: 0,
        frame_count: 0,
        channel_count: 0,
    };
    let ext_ptr = ext_c
        .as_ref()
        .map(|value| value.as_ptr())
        .unwrap_or(std::ptr::null());
    let status = crate::moodbar_native_analysis_from_bytes(
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
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
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
    let status = crate::moodbar_native_render_svg(handle as u64, opts.as_ptr(), &mut out);
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
        Err(error) => return response(&mut env, json!({"ok": false, "status": 1, "error": error})),
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
    let status = crate::moodbar_native_render_png(handle as u64, opts.as_ptr(), &mut out);
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
        crate::moodbar_native_buffer_free(&mut out);
    }
    response(&mut env, json!({"ok": true, "pngBase64": encoded}))
}

#[no_mangle]
pub extern "system" fn Java_expo_modules_moodbarnative_NativeBridge_nativeDisposeAnalysis(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jstring {
    let status = crate::moodbar_native_analysis_dispose(handle as u64);
    if (status as i32) != 0 {
        return response(&mut env, last_error_json());
    }
    response(&mut env, json!({"ok": true}))
}
