use std::ffi::{CString, c_char};

use shiguredo_rtmp::RtmpPlayClientConnection;

use crate::error::RtmpCError;
use crate::event::RtmpCEvent;
use crate::types::RtmpCConnectionState;
use crate::url::RtmpCUrl;

/// RTMP 再生クライアント（opaque ポインタ）
pub struct RtmpPlayClient {
    inner: RtmpPlayClientConnection,
    last_error: Option<CString>,
}

impl RtmpPlayClient {
    fn set_last_error(&mut self, msg: &str) {
        self.last_error = Some(CString::new(msg).unwrap_or_default());
    }
}

/// 新しい RTMP 再生クライアントを作成する
///
/// url の所有権はこの関数に移るため、呼び出し後に rtmp_url_free を呼ぶ必要はない。
/// NULL が渡された場合は NULL を返す。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_new(url: *mut RtmpCUrl) -> *mut RtmpPlayClient {
    if url.is_null() {
        return std::ptr::null_mut();
    }

    let url = unsafe { Box::from_raw(url) };
    let client = Box::new(RtmpPlayClient {
        inner: RtmpPlayClientConnection::new(url.inner),
        last_error: None,
    });
    Box::into_raw(client)
}

/// RTMP 再生クライアントを解放する
///
/// NULL ポインタが渡された場合は何もしない
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_free(client: *mut RtmpPlayClient) {
    if !client.is_null() {
        let _ = unsafe { Box::from_raw(client) };
    }
}

/// 最後に発生したエラーのメッセージを取得する
///
/// エラーが発生していない場合は空文字列を返す。
/// 返されるポインタは次のエラーが発生するか、クライアントが解放されるまで有効。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_get_last_error(
    client: *const RtmpPlayClient,
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
/// * `client` - RTMP 再生クライアント
/// * `data` - 受信データへのポインタ
/// * `len` - 受信データの長さ
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_feed_recv_buf(
    client: *mut RtmpPlayClient,
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
/// * `client` - RTMP 再生クライアント
/// * `len` - データ長の格納先
///
/// # 戻り値
///
/// 送信データへのポインタ。データがない場合はポインタは不定で len が 0 になる。
/// 返されるポインタは次にクライアントのメソッドが呼ばれるまで有効。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_send_buf(
    client: *const RtmpPlayClient,
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
pub unsafe extern "C" fn rtmp_play_client_advance_send_buf(client: *mut RtmpPlayClient, n: usize) {
    if client.is_null() {
        return;
    }
    let client = unsafe { &mut *client };
    client.inner.advance_send_buf(n);
}

/// コネクションの現在の状態を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_state(
    client: *const RtmpPlayClient,
) -> RtmpCConnectionState {
    if client.is_null() {
        return RtmpCConnectionState::RTMP_STATE_DISCONNECTING;
    }
    let client = unsafe { &*client };
    client.inner.state().into()
}

/// 次のイベントを取得する
///
/// イベントがない場合は kind が RTMP_EVENT_NONE の RtmpCEvent を返す。
/// 返された RtmpCEvent は rtmp_event_free で解放する必要がある。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_play_client_next_event(
    client: *mut RtmpPlayClient,
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
