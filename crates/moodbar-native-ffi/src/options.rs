use std::ffi::{c_char, CStr};

use moodbar_analysis::{GenerateOptions, PngOptions, SvgOptions};
use moodbar_bindings_schema::{
    apply_generate_patch, apply_png_patch, apply_svg_patch, GenerateOptionsPatch, PngOptionsPatch,
    SvgOptionsPatch,
};
use serde::Deserialize;

use crate::errors::FfiError;

pub(crate) fn parse_generate_options(
    options_json: *const c_char,
) -> Result<GenerateOptions, FfiError> {
    let input: GenerateOptionsPatch = parse_optional_json(options_json)?;
    let mut options = GenerateOptions::default();
    apply_generate_patch(&mut options, input);
    Ok(options)
}

pub(crate) fn parse_svg_options(options_json: *const c_char) -> Result<SvgOptions, FfiError> {
    let input: SvgOptionsPatch = parse_optional_json(options_json)?;
    let mut options = SvgOptions::default();
    apply_svg_patch(&mut options, input).map_err(FfiError::InvalidArgument)?;
    Ok(options)
}

pub(crate) fn parse_png_options(options_json: *const c_char) -> Result<PngOptions, FfiError> {
    let input: PngOptionsPatch = parse_optional_json(options_json)?;
    let mut options = PngOptions::default();
    apply_png_patch(&mut options, input);
    Ok(options)
}

pub(crate) fn require_c_string<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, FfiError> {
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

pub(crate) fn optional_c_string<'a>(ptr: *const c_char) -> Result<Option<&'a str>, FfiError> {
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
