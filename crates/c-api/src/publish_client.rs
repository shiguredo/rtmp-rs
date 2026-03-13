use std::ffi::{CString, c_char};

use shiguredo_rtmp::{
    AudioFrame, RtmpPublishClientConnection, RtmpTimestamp, RtmpTimestampDelta, VideoFrame,
};

use crate::error::RtmpCError;
use crate::event::RtmpCEvent;
use crate::types::{RtmpCAudioFrame, RtmpCConnectionState, RtmpCVideoFrame};
use crate::url::RtmpCUrl;

/// RTMP 配信クライアント（opaque ポインタ）
pub struct RtmpPublishClient {
    inner: RtmpPublishClientConnection,
    last_error: Option<CString>,
}

impl RtmpPublishClient {
    fn set_last_error(&mut self, msg: &str) {
        self.last_error = Some(CString::new(msg).unwrap_or_default());
    }
}

/// 新しい RTMP 配信クライアントを作成する
///
/// url の所有権はこの関数に移るため、呼び出し後に rtmp_url_free を呼ぶ必要はない。
/// NULL が渡された場合は NULL を返す。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_new(url: *mut RtmpCUrl) -> *mut RtmpPublishClient {
    if url.is_null() {
        return std::ptr::null_mut();
    }

    let url = unsafe { Box::from_raw(url) };
    let client = Box::new(RtmpPublishClient {
        inner: RtmpPublishClientConnection::new(url.inner),
        last_error: None,
    });
    Box::into_raw(client)
}

/// RTMP 配信クライアントを解放する
///
/// NULL ポインタが渡された場合は何もしない
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_free(client: *mut RtmpPublishClient) {
    if !client.is_null() {
        let _ = unsafe { Box::from_raw(client) };
    }
}

/// 最後に発生したエラーのメッセージを取得する
///
/// エラーが発生していない場合は空文字列を返す。
/// 返されるポインタは次のエラーが発生するか、クライアントが解放されるまで有効。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_get_last_error(
    client: *const RtmpPublishClient,
) -> *const c_char {
    if client.is_null() {
        return c"".as_ptr();
    }
    let client = unsafe { &*client };
    match &client.last_error {
        Some(e) => e.as_ptr(),
        None => c"".as_ptr(),
    }
}

/// サーバーから受信したデータを処理する
///
/// # 引数
///
/// * `client` - RTMP 配信クライアント
/// * `data` - 受信データへのポインタ
/// * `len` - 受信データの長さ
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_feed_recv_buf(
    client: *mut RtmpPublishClient,
    data: *const u8,
    len: usize,
) -> RtmpCError {
    if client.is_null() {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }
    if data.is_null() && len > 0 {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let client = unsafe { &mut *client };
    let buf = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };

    match client.inner.feed_recv_buf(buf) {
        Ok(()) => RtmpCError::RTMP_ERROR_OK,
        Err(e) => {
            let msg = e.to_string();
            let error_code = RtmpCError::from(e);
            client.set_last_error(&msg);
            error_code
        }
    }
}

/// サーバーに送信待ちのデータを取得する
///
/// # 引数
///
/// * `client` - RTMP 配信クライアント
/// * `len` - データ長の格納先
///
/// # 戻り値
///
/// 送信データへのポインタ。データがない場合はポインタは不定で len が 0 になる。
/// 返されるポインタは次にクライアントのメソッドが呼ばれるまで有効。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_send_buf(
    client: *const RtmpPublishClient,
    len: *mut usize,
) -> *const u8 {
    if client.is_null() || len.is_null() {
        if !len.is_null() {
            unsafe { *len = 0 };
        }
        return std::ptr::null();
    }

    let client = unsafe { &*client };
    let buf = client.inner.send_buf();
    unsafe { *len = buf.len() };
    buf.as_ptr()
}

/// 送信バッファから指定バイト数を送信済みとしてマークする
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_advance_send_buf(
    client: *mut RtmpPublishClient,
    n: usize,
) {
    if client.is_null() {
        return;
    }
    let client = unsafe { &mut *client };
    client.inner.advance_send_buf(n);
}

/// コネクションの現在の状態を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_state(
    client: *const RtmpPublishClient,
) -> RtmpCConnectionState {
    if client.is_null() {
        return RtmpCConnectionState::RTMP_STATE_DISCONNECTING;
    }
    let client = unsafe { &*client };
    client.inner.state().into()
}

/// 音声フレームを送信する
///
/// frame のデータポインタが指すメモリは、この関数の呼び出し中のみ有効であればよい（内部でコピーされる）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_send_audio(
    client: *mut RtmpPublishClient,
    frame: *const RtmpCAudioFrame,
) -> RtmpCError {
    if client.is_null() || frame.is_null() {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let client = unsafe { &mut *client };
    let frame = unsafe { &*frame };

    if frame.data.is_null() && frame.data_len > 0 {
        client.set_last_error("rtmp_publish_client_send_audio: frame data is null");
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let data = if frame.data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(frame.data, frame.data_len) }.to_vec()
    };

    let audio_frame = AudioFrame {
        timestamp: RtmpTimestamp::from_millis(frame.timestamp_millis),
        format: frame.format.into(),
        sample_rate: frame.sample_rate.into(),
        is_8bit_sample: frame.is_8bit_sample,
        is_stereo: frame.is_stereo,
        is_aac_sequence_header: frame.is_aac_sequence_header,
        data,
    };

    match client.inner.send_audio(audio_frame) {
        Ok(()) => RtmpCError::RTMP_ERROR_OK,
        Err(e) => {
            let msg = e.to_string();
            let error_code = RtmpCError::from(e);
            client.set_last_error(&msg);
            error_code
        }
    }
}

/// 映像フレームを送信する
///
/// frame のデータポインタが指すメモリは、この関数の呼び出し中のみ有効であればよい（内部でコピーされる）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_send_video(
    client: *mut RtmpPublishClient,
    frame: *const RtmpCVideoFrame,
) -> RtmpCError {
    if client.is_null() || frame.is_null() {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let client = unsafe { &mut *client };
    let frame = unsafe { &*frame };

    if frame.data.is_null() && frame.data_len > 0 {
        client.set_last_error("rtmp_publish_client_send_video: frame data is null");
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let data = if frame.data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(frame.data, frame.data_len) }.to_vec()
    };

    let avc_packet_type = if frame.has_avc_packet_type {
        Some(frame.avc_packet_type.into())
    } else {
        None
    };

    let video_frame = VideoFrame {
        timestamp: RtmpTimestamp::from_millis(frame.timestamp_millis),
        composition_timestamp_offset: RtmpTimestampDelta::from_millis(
            frame.composition_timestamp_offset_millis,
        ),
        frame_type: frame.frame_type.into(),
        codec: frame.codec.into(),
        avc_packet_type,
        data,
    };

    match client.inner.send_video(video_frame) {
        Ok(()) => RtmpCError::RTMP_ERROR_OK,
        Err(e) => {
            let msg = e.to_string();
            let error_code = RtmpCError::from(e);
            client.set_last_error(&msg);
            error_code
        }
    }
}

/// 次のイベントを取得する
///
/// イベントがない場合は kind が RTMP_EVENT_NONE の RtmpCEvent を返す。
/// 返された RtmpCEvent は rtmp_event_free で解放する必要がある。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_publish_client_next_event(
    client: *mut RtmpPublishClient,
) -> *mut RtmpCEvent {
    if client.is_null() {
        return Box::into_raw(Box::new(RtmpCEvent::none()));
    }

    let client = unsafe { &mut *client };
    let event = match client.inner.next_event() {
        Some(event) => RtmpCEvent::from_event(event),
        None => RtmpCEvent::none(),
    };
    Box::into_raw(Box::new(event))
}
