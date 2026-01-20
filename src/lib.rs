//! RTMP (Real Time Messaging Protocol) の Sans I/O 実装を提供するライブラリ
#![cfg_attr(not(feature = "pbt"), warn(missing_docs))]
mod amf;
mod amf0;
mod amf3;
mod bytes;
mod error;
mod flv;
mod media;
mod rtmp_chunk;
mod rtmp_chunk_decoder;
mod rtmp_chunk_encoder;
mod rtmp_client_connection;
mod rtmp_command;
mod rtmp_connection;
mod rtmp_handshake;
mod rtmp_message;
mod rtmp_message_decoder;
mod rtmp_message_encoder;
mod rtmp_server_connection;
mod rtmp_timestamp;
mod rtmp_user_control_event;

pub use error::{Error, ErrorKind};
pub use media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, MediaFrame, VideoCodec, VideoFrame,
    VideoFrameType,
};
pub use rtmp_client_connection::{RtmpPlayClientConnection, RtmpPublishClientConnection};
pub use rtmp_connection::{RtmpConnectionEvent, RtmpConnectionState};
pub use rtmp_server_connection::RtmpServerConnection;
pub use rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};

// PBT / Fuzzing 用に条件付きで公開しているモジュール
#[cfg(feature = "pbt")]
pub mod tests {
    pub use crate::amf::*;
    pub use crate::amf0::*;
    pub use crate::amf3::*;
    pub use crate::flv::*;
    pub use crate::media::*;
    pub use crate::rtmp_chunk::*;
    pub use crate::rtmp_chunk_decoder::*;
    pub use crate::rtmp_chunk_encoder::*;
    pub use crate::rtmp_client_connection::*;
    pub use crate::rtmp_command::TransactionId;
    pub use crate::rtmp_command::*;
    pub use crate::rtmp_connection::*;
    pub use crate::rtmp_handshake::*;
    pub use crate::rtmp_message::*;
    pub use crate::rtmp_message_decoder::*;
    pub use crate::rtmp_message_encoder::*;
    pub use crate::rtmp_server_connection::*;
    pub use crate::rtmp_timestamp::*;
    pub use crate::rtmp_user_control_event::*;
}
