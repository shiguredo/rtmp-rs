use std::ffi::{CStr, CString, c_char};
use std::ptr;

use crate::error::{RtmpError, clear_last_error, invalid_input, null_pointer};

pub(crate) fn cstring_lossy<S: AsRef<str>>(value: S) -> CString {
    let bytes = value
        .as_ref()
        .as_bytes()
        .iter()
        .map(|byte| if *byte == 0 { b' ' } else { *byte })
        .collect::<Vec<_>>();
    CString::new(bytes).expect("interior NUL bytes are replaced with spaces")
}

pub(crate) unsafe fn ref_arg<'a, T>(ptr: *const T, name: &str) -> Result<&'a T, RtmpError> {
    if ptr.is_null() {
        return Err(null_pointer(name));
    }
    Ok(unsafe { &*ptr })
}

pub(crate) unsafe fn mut_arg<'a, T>(ptr: *mut T, name: &str) -> Result<&'a mut T, RtmpError> {
    if ptr.is_null() {
        return Err(null_pointer(name));
    }
    Ok(unsafe { &mut *ptr })
}

pub(crate) unsafe fn cstr_arg<'a>(ptr: *const c_char, name: &str) -> Result<&'a CStr, RtmpError> {
    if ptr.is_null() {
        return Err(null_pointer(name));
    }
    Ok(unsafe { CStr::from_ptr(ptr) })
}

pub(crate) unsafe fn str_arg<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, RtmpError> {
    let cstr = unsafe { cstr_arg(ptr, name) }?;
    cstr.to_str()
        .map_err(|e| invalid_input(format!("{name} must be valid UTF-8: {e}")))
}

pub(crate) unsafe fn slice_arg<'a>(
    ptr: *const u8,
    len: usize,
    name: &str,
) -> Result<&'a [u8], RtmpError> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(null_pointer(name));
    }
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

pub(crate) unsafe fn write_raw_ptr_out<T>(
    out: *mut *mut T,
    value: *mut T,
) -> Result<(), RtmpError> {
    if out.is_null() {
        return Err(null_pointer("out"));
    }
    unsafe {
        ptr::write(out, value);
    }
    clear_last_error();
    Ok(())
}

pub(crate) unsafe fn write_box_out<T>(out: *mut *mut T, value: T) -> Result<(), RtmpError> {
    unsafe { write_raw_ptr_out(out, Box::into_raw(Box::new(value))) }
}

pub(crate) unsafe fn write_copy_out<T>(out: *mut T, value: T) -> Result<(), RtmpError> {
    if out.is_null() {
        return Err(null_pointer("out"));
    }
    unsafe {
        ptr::write(out, value);
    }
    clear_last_error();
    Ok(())
}

pub(crate) unsafe fn write_bytes_out(
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    bytes: &[u8],
) -> Result<(), RtmpError> {
    if out_ptr.is_null() {
        return Err(null_pointer("out_ptr"));
    }
    if out_len.is_null() {
        return Err(null_pointer("out_len"));
    }
    unsafe {
        ptr::write(out_ptr, ptr_or_null(bytes));
        ptr::write(out_len, bytes.len());
    }
    clear_last_error();
    Ok(())
}

pub(crate) fn ptr_or_null(bytes: &[u8]) -> *const u8 {
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}
