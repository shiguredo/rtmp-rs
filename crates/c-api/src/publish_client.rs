use std::ffi::c_char;

use shiguredo_rtmp::{RtmpPublishClientConnection as InnerPublishClientConnection, RtmpUrl};

use crate::audio_frame::RtmpAudioFrame;
use crate::basic_types::RtmpConnectionState;
use crate::connection_event::RtmpConnectionEvent;
use crate::error::{RtmpError, map_result, ok, set_last_error};
use crate::util::{
    mut_arg, ref_arg, slice_arg, str_arg, write_box_out, write_bytes_out, write_copy_out,
    write_raw_ptr_out,
};
use crate::video_frame::RtmpVideoFrame;

/// 配信用クライアント接続の opaque ハンドル
pub struct RtmpPublishClientConnection {
    inner: InnerPublishClientConnection,
}

/// 配信用クライアント接続を作成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_connection_new(
    url: *const c_char,
    out: *mut *mut RtmpPublishClientConnection,
) -> RtmpError {
    let url = match unsafe { str_arg(url, "url") } {
        Ok(url) => url,
        Err(error) => return error,
    };
    let url = match RtmpUrl::parse(url) {
        Ok(url) => url,
        Err(error) => return set_last_error(&error),
    };
    let connection = RtmpPublishClientConnection {
        inner: InnerPublishClientConnection::new(url),
    };
    match unsafe { write_box_out(out, connection) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 配信用クライアント接続を解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_connection_free(
    connection: *mut RtmpPublishClientConnection,
) {
    if !connection.is_null() {
        let _ = unsafe { Box::from_raw(connection) };
    }
}

/// 受信データを処理する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_connection_feed_recv_buf(
    connection: *mut RtmpPublishClientConnection,
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
pub unsafe extern "C" fn rtmp_publish_client_connection_get_send_buf(
    connection: *const RtmpPublishClientConnection,
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
pub unsafe extern "C" fn rtmp_publish_client_connection_advance_send_buf(
    connection: *mut RtmpPublishClientConnection,
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
pub unsafe extern "C" fn rtmp_publish_client_connection_state(
    connection: *const RtmpPublishClientConnection,
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

/// 音声フレームを送信する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_connection_send_audio(
    connection: *mut RtmpPublishClientConnection,
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
pub unsafe extern "C" fn rtmp_publish_client_connection_send_video(
    connection: *mut RtmpPublishClientConnection,
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
pub unsafe extern "C" fn rtmp_publish_client_connection_next_event(
    connection: *mut RtmpPublishClientConnection,
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
