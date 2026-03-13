use std::ffi::c_char;

use shiguredo_rtmp::{RtmpPlayClientConnection as InnerPlayClientConnection, RtmpUrl};

use crate::basic_types::RtmpConnectionState;
use crate::connection_event::RtmpConnectionEvent;
use crate::error::{RtmpError, map_result, ok, set_last_error};
use crate::util::{
    mut_arg, ref_arg, slice_arg, str_arg, write_box_out, write_bytes_out, write_copy_out,
    write_raw_ptr_out,
};

/// 再生用クライアント接続の opaque ハンドル
pub struct RtmpPlayClientConnection {
    inner: InnerPlayClientConnection,
}

/// 再生用クライアント接続を作成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_new(
    url: *const c_char,
    out: *mut *mut RtmpPlayClientConnection,
) -> RtmpError {
    let url = match unsafe { str_arg(url, "url") } {
        Ok(url) => url,
        Err(error) => return error,
    };
    let url = match RtmpUrl::parse(url) {
        Ok(url) => url,
        Err(error) => return set_last_error(&error),
    };
    let connection = RtmpPlayClientConnection {
        inner: InnerPlayClientConnection::new(url),
    };
    match unsafe { write_box_out(out, connection) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 再生用クライアント接続を解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_free(
    connection: *mut RtmpPlayClientConnection,
) {
    if !connection.is_null() {
        let _ = unsafe { Box::from_raw(connection) };
    }
}

/// 受信データを処理する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_feed_recv_buf(
    connection: *mut RtmpPlayClientConnection,
    buf: *const u8,
    buf_len: usize,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    let buf = match unsafe { slice_arg(buf, buf_len, "buf") } {
        Ok(buf) => buf,
        Err(error) => return error,
    };
    map_result(connection.inner.feed_recv_buf(buf))
}

/// 送信バッファを取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_get_send_buf(
    connection: *const RtmpPlayClientConnection,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> RtmpError {
    let connection = match unsafe { ref_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    match unsafe { write_bytes_out(out_ptr, out_len, connection.inner.send_buf()) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 送信済みバイト数を進める
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_advance_send_buf(
    connection: *mut RtmpPlayClientConnection,
    n: usize,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    connection.inner.advance_send_buf(n);
    ok()
}

/// 現在の状態を取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_state(
    connection: *const RtmpPlayClientConnection,
    out: *mut RtmpConnectionState,
) -> RtmpError {
    let connection = match unsafe { ref_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    match unsafe { write_copy_out(out, connection.inner.state().into()) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 次のイベントを取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_connection_next_event(
    connection: *mut RtmpPlayClientConnection,
    out: *mut *mut RtmpConnectionEvent,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    let event = connection
        .inner
        .next_event()
        .map(RtmpConnectionEvent::from_inner)
        .map(|event| Box::into_raw(Box::new(event)))
        .unwrap_or(std::ptr::null_mut());
    match unsafe { write_raw_ptr_out(out, event) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}
