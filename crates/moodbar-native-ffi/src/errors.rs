use std::cell::RefCell;

use moodbar_analysis::MoodbarError;
use moodbar_decode::MoodbarDecodeError;
use thiserror::Error;

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
            FfiError::Utf8 | FfiError::Json(_) => Self::InvalidArgument,
            FfiError::Poisoned | FfiError::Panic => Self::Internal,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum FfiError {
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

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

fn set_last_error(message: String) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = message;
    });
}

fn clear_last_error() {
    set_last_error(String::new());
}

pub(crate) fn last_error_message() -> String {
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

pub(crate) fn ffi_guard(f: impl FnOnce() -> Result<(), FfiError>) -> MoodbarNativeStatus {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    match caught {
        Ok(result) => ffi_status_from_result(result),
        Err(_) => ffi_status_from_result(Err(FfiError::Panic)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_not_found(message: &str) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::NotFound, message.to_string())
    }

    fn io_other(message: &str) -> std::io::Error {
        std::io::Error::other(message.to_string())
    }

    #[test]
    fn ffi_guard_success_returns_ok_and_clears_last_error() {
        let status = ffi_guard(|| Err(FfiError::InvalidArgument("bad input".to_string())));
        assert_eq!(status as i32, MoodbarNativeStatus::InvalidArgument as i32);
        assert!(!last_error_message().is_empty());

        let status = ffi_guard(|| Ok(()));
        assert_eq!(status as i32, MoodbarNativeStatus::Ok as i32);
        assert!(last_error_message().is_empty());
    }

    #[test]
    fn ffi_guard_failure_sets_last_error_and_expected_statuses() {
        let cases = vec![
            (
                FfiError::InvalidArgument("invalid".to_string()),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::NotFound("missing".to_string()),
                MoodbarNativeStatus::NotFound,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::NoAudioTrack),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::EmptyAudio),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::InvalidOptions("bad".to_string())),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::Io(io_not_found("no file"))),
                MoodbarNativeStatus::NotFound,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::Io(io_other("disk failure"))),
                MoodbarNativeStatus::Internal,
            ),
            (
                FfiError::Analysis(MoodbarError::InvalidOptions(
                    "bad render options".to_string(),
                )),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (FfiError::Utf8, MoodbarNativeStatus::InvalidArgument),
            (
                FfiError::Json("invalid json".to_string()),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (FfiError::Poisoned, MoodbarNativeStatus::Internal),
            (FfiError::Panic, MoodbarNativeStatus::Internal),
        ];

        for (error, expected_status) in cases {
            let status = ffi_guard(|| Err(error));
            assert_eq!(status as i32, expected_status as i32);
            assert!(!last_error_message().is_empty());
        }
    }

    #[test]
    fn ffi_guard_success_after_failure_clears_previous_error() {
        let status = ffi_guard(|| Err(FfiError::Json("bad json".to_string())));
        assert_eq!(status as i32, MoodbarNativeStatus::InvalidArgument as i32);
        assert!(!last_error_message().is_empty());

        let status = ffi_guard(|| Ok(()));
        assert_eq!(status as i32, MoodbarNativeStatus::Ok as i32);
        assert!(last_error_message().is_empty());
    }

    #[test]
    fn ffi_guard_panics_return_internal_and_set_last_error() {
        let status = ffi_guard(|| -> Result<(), FfiError> {
            panic!("boom");
        });
        assert_eq!(status as i32, MoodbarNativeStatus::Internal as i32);
        let message = last_error_message();
        assert!(message.contains("panic across FFI boundary"));
    }

    #[test]
    fn from_error_maps_variants_to_expected_status() {
        let cases = vec![
            (
                FfiError::InvalidArgument("bad".to_string()),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::NotFound("missing".to_string()),
                MoodbarNativeStatus::NotFound,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::NoAudioTrack),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::Io(io_not_found("missing"))),
                MoodbarNativeStatus::NotFound,
            ),
            (
                FfiError::Decode(MoodbarDecodeError::Io(io_other("broken"))),
                MoodbarNativeStatus::Internal,
            ),
            (
                FfiError::Analysis(MoodbarError::InvalidOptions("bad".to_string())),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (FfiError::Utf8, MoodbarNativeStatus::InvalidArgument),
            (
                FfiError::Json("bad".to_string()),
                MoodbarNativeStatus::InvalidArgument,
            ),
            (FfiError::Poisoned, MoodbarNativeStatus::Internal),
            (FfiError::Panic, MoodbarNativeStatus::Internal),
        ];

        for (error, expected) in cases {
            assert_eq!(
                MoodbarNativeStatus::from_error(&error) as i32,
                expected as i32
            );
        }
    }
}
