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
            FfiError::Poisoned | FfiError::Panic | FfiError::Utf8 | FfiError::Json(_) => {
                Self::Internal
            }
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
