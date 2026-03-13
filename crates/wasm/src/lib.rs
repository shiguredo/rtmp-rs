//! shiguredo_rtmp の wasm バインディング
//!
//! c-api の機能に加えて、wasm 固有の JSON 変換機能を提供する。
#![warn(missing_docs)]
#![expect(clippy::missing_safety_doc)]

/// 音声フレームの JSON 変換 API
pub mod audio_frame;
/// 接続イベントの JSON 変換 API
pub mod connection_event;
/// 映像フレームの JSON 変換 API
pub mod video_frame;

use std::alloc::Layout;

/// メモリを確保する
#[unsafe(no_mangle)]
pub extern "C" fn rtmp_alloc(size: u32) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    let layout = Layout::from_size_align(size as usize, 1)
        .expect("layout creation with alignment 1 should never fail");
    unsafe { std::alloc::alloc(layout) }
}

/// `rtmp_alloc()` で確保したメモリを解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_free(ptr: *mut u8, size: u32) {
    if ptr.is_null() || size == 0 {
        return;
    }
    let layout = Layout::from_size_align(size as usize, 1)
        .expect("layout creation with alignment 1 should never fail");
    unsafe { std::alloc::dealloc(ptr, layout) };
}

/// `Vec<u8>` の先頭ポインタを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_vec_ptr(v: *const Vec<u8>) -> *const u8 {
    if v.is_null() {
        return std::ptr::null();
    }
    unsafe { (*v).as_ptr() }
}

/// `Vec<u8>` の長さを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_vec_len(v: *const Vec<u8>) -> u32 {
    if v.is_null() {
        return 0;
    }
    unsafe {
        u32::try_from((*v).len()).expect("vec length should not exceed u32::MAX in wasm32 target")
    }
}

/// `Vec<u8>` を解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_vec_free(v: *mut Vec<u8>) {
    if !v.is_null() {
        let _ = unsafe { Box::from_raw(v) };
    }
}
