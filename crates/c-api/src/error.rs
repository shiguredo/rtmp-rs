use std::cell::RefCell;
use std::ffi::{CString, c_char};

use shiguredo_rtmp::{Error, ErrorKind};

/// C API で返すエラーコード
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpError {
    RTMP_ERROR_OK = 0,
    RTMP_ERROR_INVALID_INPUT,
    RTMP_ERROR_INVALID_DATA,
    RTMP_ERROR_INVALID_STATE,
    RTMP_ERROR_UNSUPPORTED,
    RTMP_ERROR_NULL_POINTER,
    RTMP_ERROR_OTHER,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

impl From<ErrorKind> for RtmpError {
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::InvalidInput => Self::RTMP_ERROR_INVALID_INPUT,
            ErrorKind::InvalidData => Self::RTMP_ERROR_INVALID_DATA,
            ErrorKind::InvalidState => Self::RTMP_ERROR_INVALID_STATE,
            ErrorKind::Unsupported => Self::RTMP_ERROR_UNSUPPORTED,
            ErrorKind::InsufficientBuffer => Self::RTMP_ERROR_INVALID_DATA,
            _ => Self::RTMP_ERROR_OTHER,
        }
    }
}

impl From<&Error> for RtmpError {
    fn from(error: &Error) -> Self {
        error.kind.into()
    }
}

pub(crate) fn ok() -> RtmpError {
    clear_last_error();
    RtmpError::RTMP_ERROR_OK
}

pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|message| {
        *message.borrow_mut() = None;
    });
}

pub(crate) fn set_last_error_message<T: Into<String>>(code: RtmpError, message: T) -> RtmpError {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = Some(crate::util::cstring_lossy(message.into()));
    });
    code
}

pub(crate) fn set_last_error(error: &Error) -> RtmpError {
    set_last_error_message(error.into(), error.to_string())
}

pub(crate) fn null_pointer(name: &str) -> RtmpError {
    set_last_error_message(
        RtmpError::RTMP_ERROR_NULL_POINTER,
        format!("{name} must not be null"),
    )
}

pub(crate) fn invalid_input<T: Into<String>>(message: T) -> RtmpError {
    set_last_error_message(RtmpError::RTMP_ERROR_INVALID_INPUT, message)
}

pub(crate) fn map_result(result: Result<(), Error>) -> RtmpError {
    match result {
        Ok(()) => ok(),
        Err(error) => set_last_error(&error),
    }
}

/// 直近のエラーメッセージを取得する
///
/// 返されるポインタは次に同一スレッドで C API が呼ばれるまで有効です。
#[unsafe(no_mangle)]
pub extern "C" fn rtmp_last_error_message() -> *const c_char {
    LAST_ERROR.with(|message| {
        message
            .borrow()
            .as_ref()
            .map_or(std::ptr::null(), |message| message.as_ptr())
    })
}

/// 直近のエラーメッセージをクリアする
#[unsafe(no_mangle)]
pub extern "C" fn rtmp_clear_last_error_message() {
    clear_last_error();
}
