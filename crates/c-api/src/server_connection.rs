use std::ffi::c_char;

use shiguredo_rtmp::RtmpServerConnection as InnerServerConnection;

use crate::audio_frame::RtmpAudioFrame;
use crate::basic_types::RtmpConnectionState;
use crate::connection_event::RtmpConnectionEvent;
use crate::error::{RtmpError, map_result, ok};
use crate::util::{
    mut_arg, ref_arg, slice_arg, str_arg, write_box_out, write_bytes_out, write_copy_out,
    write_raw_ptr_out,
};
use crate::video_frame::RtmpVideoFrame;

/// サーバー接続の opaque ハンドル
pub struct RtmpServerConnection {
    inner: InnerServerConnection,
}

/// サーバー接続を作成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_new(
    out: *mut *mut RtmpServerConnection,
) -> RtmpError {
    let connection = RtmpServerConnection {
        inner: InnerServerConnection::new(),
    };
    match unsafe { write_box_out(out, connection) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// サーバー接続を解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_free(connection: *mut RtmpServerConnection) {
    if !connection.is_null() {
        let _ = unsafe { Box::from_raw(connection) };
    }
}

/// 受信データを処理する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_feed_recv_buf(
    connection: *mut RtmpServerConnection,
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
pub unsafe extern "C" fn rtmp_server_connection_get_send_buf(
    connection: *const RtmpServerConnection,
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
pub unsafe extern "C" fn rtmp_server_connection_advance_send_buf(
    connection: *mut RtmpServerConnection,
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
pub unsafe extern "C" fn rtmp_server_connection_state(
    connection: *const RtmpServerConnection,
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

/// 配信 / 再生要求を受理する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_accept(
    connection: *mut RtmpServerConnection,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    map_result(connection.inner.accept())
}

/// 配信 / 再生要求を拒否する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_reject(
    connection: *mut RtmpServerConnection,
    reason: *const c_char,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    let reason = match unsafe { str_arg(reason, "reason") } {
        Ok(reason) => reason,
        Err(error) => return error,
    };
    map_result(connection.inner.reject(reason))
}

/// 音声フレームを送信する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_send_audio(
    connection: *mut RtmpServerConnection,
    frame: *const RtmpAudioFrame,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    let frame = match unsafe { ref_arg(frame, "frame") } {
        Ok(frame) => frame,
        Err(error) => return error,
    };
    map_result(connection.inner.send_audio(frame.clone_inner()))
}

/// 映像フレームを送信する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_send_video(
    connection: *mut RtmpServerConnection,
    frame: *const RtmpVideoFrame,
) -> RtmpError {
    let connection = match unsafe { mut_arg(connection, "connection") } {
        Ok(connection) => connection,
        Err(error) => return error,
    };
    let frame = match unsafe { ref_arg(frame, "frame") } {
        Ok(frame) => frame,
        Err(error) => return error,
    };
    map_result(connection.inner.send_video(frame.clone_inner()))
}

/// 次のイベントを取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_server_connection_next_event(
    connection: *mut RtmpServerConnection,
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
