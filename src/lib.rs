//! RTMP (Real Time Messaging Protocol) の Sans I/O 実装を提供するライブラリ
//!
//! I/O は行わず、バイト列の入出力と状態遷移のみを扱う。
//! RTMP のチャンク、メッセージ、AMF0/AMF3、FLV、ハンドシェイク、
//! クライアント接続、サーバー接続をモジュール単位で公開している。
#![no_std]
#![warn(missing_docs)]

#[macro_use]
extern crate alloc;

pub mod amf;
pub mod amf0;
pub mod amf3;
mod bytes;
pub mod error;
pub mod flv;
pub mod media;
pub mod rtmp_chunk;
pub mod rtmp_chunk_decoder;
pub mod rtmp_chunk_encoder;
pub mod rtmp_client_connection;
pub mod rtmp_command;
pub mod rtmp_connection;
pub mod rtmp_handshake;
pub mod rtmp_message;
pub mod rtmp_message_decoder;
pub mod rtmp_message_encoder;
pub mod rtmp_server_connection;
pub mod rtmp_timestamp;
pub mod rtmp_url;
pub mod rtmp_user_control_event;

pub use error::{Error, ErrorKind};
pub use media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, AvcSequenceHeader, MediaFrame,
    VideoCodec, VideoFrame, VideoFrameType,
};
pub use rtmp_client_connection::{RtmpPlayClientConnection, RtmpPublishClientConnection};
pub use rtmp_connection::{RtmpConnectionEvent, RtmpConnectionState};
pub use rtmp_server_connection::RtmpServerConnection;
pub use rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
pub use rtmp_url::RtmpUrl;
