#![expect(clippy::missing_safety_doc)]

pub mod audio_frame;
pub mod basic_types;
pub mod connection_event;
pub mod error;
pub mod play_client;
pub mod publish_client;
pub mod server_connection;
pub mod video_frame;

mod util;

/// ライブラリのバージョンを取得する
#[unsafe(no_mangle)]
pub extern "C" fn rtmp_library_version() -> *const std::ffi::c_char {
    concat!(env!("SHIGUREDO_RTMP_VERSION"), "\0")
        .as_ptr()
        .cast()
}
