use std::ffi::{CStr, CString, c_char};

use shiguredo_rtmp::RtmpUrl;

use crate::error::RtmpCError;

/// C 側に公開する RtmpUrl のラッパー（opaque ポインタ）
pub struct RtmpCUrl {
    pub(crate) inner: RtmpUrl,
    host_cache: CString,
    app_cache: CString,
    stream_name_cache: CString,
}

impl RtmpCUrl {
    fn new(inner: RtmpUrl) -> Self {
        let host_cache = CString::new(inner.host.as_str()).unwrap_or_default();
        let app_cache = CString::new(inner.app.as_str()).unwrap_or_default();
        let stream_name_cache = CString::new(inner.stream_name.as_str()).unwrap_or_default();
        Self {
            inner,
            host_cache,
            app_cache,
            stream_name_cache,
        }
    }
}

/// RTMP URL をパースする
///
/// # 引数
///
/// * `url` - パースする URL 文字列（NULL 終端）
/// * `out` - パース結果の RtmpCUrl ポインタの格納先
///
/// # 戻り値
///
/// 成功した場合は RTMP_ERROR_OK を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_parse(url: *const c_char, out: *mut *mut RtmpCUrl) -> RtmpCError {
    if url.is_null() || out.is_null() {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let url_str = match unsafe { CStr::from_ptr(url) }.to_str() {
        Ok(s) => s,
        Err(_) => return RtmpCError::RTMP_ERROR_INVALID_INPUT,
    };

    match RtmpUrl::parse(url_str) {
        Ok(parsed) => {
            let boxed = Box::new(RtmpCUrl::new(parsed));
            unsafe { *out = Box::into_raw(boxed) };
            RtmpCError::RTMP_ERROR_OK
        }
        Err(e) => e.into(),
    }
}

/// RTMP URL をストリーム名指定でパースする
///
/// URL にストリーム名が含まれない場合に使用する
///
/// # 引数
///
/// * `url` - パースする URL 文字列（NULL 終端）
/// * `stream_name` - ストリーム名（NULL 終端）
/// * `out` - パース結果の RtmpCUrl ポインタの格納先
///
/// # 戻り値
///
/// 成功した場合は RTMP_ERROR_OK を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_parse_with_stream_name(
    url: *const c_char,
    stream_name: *const c_char,
    out: *mut *mut RtmpCUrl,
) -> RtmpCError {
    if url.is_null() || stream_name.is_null() || out.is_null() {
        return RtmpCError::RTMP_ERROR_NULL_POINTER;
    }

    let url_str = match unsafe { CStr::from_ptr(url) }.to_str() {
        Ok(s) => s,
        Err(_) => return RtmpCError::RTMP_ERROR_INVALID_INPUT,
    };

    let stream_name_str = match unsafe { CStr::from_ptr(stream_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return RtmpCError::RTMP_ERROR_INVALID_INPUT,
    };

    match RtmpUrl::parse_with_stream_name(url_str, stream_name_str) {
        Ok(parsed) => {
            let boxed = Box::new(RtmpCUrl::new(parsed));
            unsafe { *out = Box::into_raw(boxed) };
            RtmpCError::RTMP_ERROR_OK
        }
        Err(e) => e.into(),
    }
}

/// RtmpCUrl を解放する
///
/// NULL ポインタが渡された場合は何もしない
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_free(url: *mut RtmpCUrl) {
    if !url.is_null() {
        let _ = unsafe { Box::from_raw(url) };
    }
}

/// ホスト名を取得する
///
/// 返されるポインタは RtmpCUrl が解放されるまで有効
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_host(url: *const RtmpCUrl) -> *const c_char {
    if url.is_null() {
        return c"".as_ptr();
    }
    let url = unsafe { &*url };
    url.host_cache.as_ptr()
}

/// ポート番号を取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_port(url: *const RtmpCUrl) -> u16 {
    if url.is_null() {
        return 0;
    }
    let url = unsafe { &*url };
    url.inner.port
}

/// アプリケーション名を取得する
///
/// 返されるポインタは RtmpCUrl が解放されるまで有効
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_app(url: *const RtmpCUrl) -> *const c_char {
    if url.is_null() {
        return c"".as_ptr();
    }
    let url = unsafe { &*url };
    url.app_cache.as_ptr()
}

/// ストリーム名を取得する
///
/// 返されるポインタは RtmpCUrl が解放されるまで有効
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_stream_name(url: *const RtmpCUrl) -> *const c_char {
    if url.is_null() {
        return c"".as_ptr();
    }
    let url = unsafe { &*url };
    url.stream_name_cache.as_ptr()
}

/// TLS が使用されるかどうかを取得する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_url_tls(url: *const RtmpCUrl) -> bool {
    if url.is_null() {
        return false;
    }
    let url = unsafe { &*url };
    url.inner.tls
}
