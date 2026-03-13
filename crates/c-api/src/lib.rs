#![expect(clippy::missing_safety_doc)]

pub mod error;
pub mod event;
pub mod play_client;
pub mod publish_client;
pub mod types;
pub mod url;

/// ライブラリのバージョンを取得する
///
/// # 戻り値
///
/// バージョン文字列へのポインタ（NULL 終端）
#[unsafe(no_mangle)]
pub extern "C" fn rtmp_library_version() -> *const std::ffi::c_char {
    concat!(env!("SHIGUREDO_RTMP_VERSION"), "\0")
        .as_ptr()
        .cast()
}
