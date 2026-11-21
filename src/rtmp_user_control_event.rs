//! RTMP のユーザーコントロールイベントをエンコード・デコードするモジュール
//! イベントの種類とデータ形式は RTMP 仕様 5.4.1 節に従う。

use alloc::vec::Vec;

use crate::bytes::BytesReader;
use crate::error::Error;
use crate::rtmp_message::RtmpMessageStreamId;
use crate::rtmp_timestamp::RtmpTimestamp;

const EVENT_STREAM_BEGIN: u16 = 0;
const EVENT_STREAM_EOF: u16 = 1;
const EVENT_STREAM_DRY: u16 = 2;
const EVENT_SET_BUFFER_LENGTH: u16 = 3;
const EVENT_STREAM_IS_RECORDED: u16 = 4;
const EVENT_PING_REQUEST: u16 = 6;
const EVENT_PING_RESPONSE: u16 = 7;
const EVENT_BUFFER_EMPTY: u16 = 31;
const EVENT_BUFFER_READY: u16 = 32;

/// RTMP のユーザーコントロールイベント
///
/// ユーザーコントロールメッセージのペイロードに含まれるイベントを表す。
/// イベントタイプは 16 ビットの符号なし整数で、イベントごとに続くデータの形式が異なる
/// （RTMP 仕様 5.4.1 節）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtmpUserControlEvent {
    /// ストリームの開始を通知するイベント
    StreamBegin {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },

    /// ストリームの終了を通知するイベント
    StreamEof {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },

    /// ストリームにデータがない状態を通知するイベント
    StreamDry {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },

    /// バッファ長を設定するイベント
    SetBufferLength {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,

        /// バッファ長（ミリ秒単位）
        length: u32,
    },

    /// ストリームが録画対象であることを通知するイベント
    StreamIsRecorded {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },

    /// ping を要求するイベント
    PingRequest {
        /// ping のタイムスタンプ（ミリ秒単位）
        timestamp: RtmpTimestamp,
    },

    /// ping に応答するイベント
    PingResponse {
        /// ping のタイムスタンプ（ミリ秒単位）
        timestamp: RtmpTimestamp,
    },

    /// バッファが空になったことを通知するイベント
    BufferEmpty {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },

    /// バッファに再生可能なデータが溜まったことを通知するイベント
    BufferReady {
        /// 対象のメッセージストリーム ID
        stream_id: RtmpMessageStreamId,
    },
}

impl RtmpUserControlEvent {
    /// イベント名を取得する
    ///
    /// RTMP 仕様 5.4.1 節で定義されているイベント名を返す。
    pub fn name(self) -> &'static str {
        match self {
            Self::StreamBegin { .. } => "StreamBegin",
            Self::StreamEof { .. } => "StreamEof",
            Self::StreamDry { .. } => "StreamDry",
            Self::SetBufferLength { .. } => "SetBufferLength",
            Self::StreamIsRecorded { .. } => "StreamIsRecorded",
            Self::PingRequest { .. } => "PingRequest",
            Self::PingResponse { .. } => "PingResponse",
            Self::BufferEmpty { .. } => "BufferEmpty",
            Self::BufferReady { .. } => "BufferReady",
        }
    }

    /// イベントをユーザーコントロールメッセージのペイロードとして `buf` の末尾に書き込む
    ///
    /// イベントタイプ（16 ビット）に続けてイベントデータをビッグエンディアンで書き込む。
    pub fn encode(self, buf: &mut Vec<u8>) {
        match self {
            Self::StreamBegin { stream_id } => {
                buf.extend_from_slice(&EVENT_STREAM_BEGIN.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
            Self::StreamEof { stream_id } => {
                buf.extend_from_slice(&EVENT_STREAM_EOF.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
            Self::StreamDry { stream_id } => {
                buf.extend_from_slice(&EVENT_STREAM_DRY.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
            Self::SetBufferLength { stream_id, length } => {
                buf.extend_from_slice(&EVENT_SET_BUFFER_LENGTH.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
                buf.extend_from_slice(&length.to_be_bytes());
            }
            Self::StreamIsRecorded { stream_id } => {
                buf.extend_from_slice(&EVENT_STREAM_IS_RECORDED.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
            Self::PingRequest { timestamp } => {
                buf.extend_from_slice(&EVENT_PING_REQUEST.to_be_bytes());
                buf.extend_from_slice(&timestamp.as_millis().to_be_bytes());
            }
            Self::PingResponse { timestamp } => {
                buf.extend_from_slice(&EVENT_PING_RESPONSE.to_be_bytes());
                buf.extend_from_slice(&timestamp.as_millis().to_be_bytes());
            }
            Self::BufferEmpty { stream_id } => {
                buf.extend_from_slice(&EVENT_BUFFER_EMPTY.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
            Self::BufferReady { stream_id } => {
                buf.extend_from_slice(&EVENT_BUFFER_READY.to_be_bytes());
                buf.extend_from_slice(&stream_id.get().to_be_bytes());
            }
        }
    }

    /// ユーザーコントロールメッセージのペイロードをデコードする
    ///
    /// `buf` の先頭からイベントタイプとイベントデータを読み取り、対応するイベントを返す。
    /// イベントデータの後ろに余分なデータがあっても無視される。
    /// 未知のイベントタイプの場合は [`Error`] を返す。
    pub fn decode(mut buf: &[u8]) -> Result<Self, Error> {
        let event_type = buf.read_u16()?;
        let event = match event_type {
            EVENT_STREAM_BEGIN => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::StreamBegin { stream_id }
            }
            EVENT_STREAM_EOF => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::StreamEof { stream_id }
            }
            EVENT_STREAM_DRY => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::StreamDry { stream_id }
            }
            EVENT_SET_BUFFER_LENGTH => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                let length = buf.read_u32()?;
                Self::SetBufferLength { stream_id, length }
            }
            EVENT_STREAM_IS_RECORDED => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::StreamIsRecorded { stream_id }
            }
            EVENT_PING_REQUEST => {
                let timestamp = RtmpTimestamp::from_millis(buf.read_u32()?);
                Self::PingRequest { timestamp }
            }
            EVENT_PING_RESPONSE => {
                let timestamp = RtmpTimestamp::from_millis(buf.read_u32()?);
                Self::PingResponse { timestamp }
            }
            EVENT_BUFFER_EMPTY => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::BufferEmpty { stream_id }
            }
            EVENT_BUFFER_READY => {
                let stream_id = RtmpMessageStreamId::new(buf.read_u32()?);
                Self::BufferReady { stream_id }
            }
            _ => {
                return Err(Error::invalid_data(format!(
                    "unknown user control event type: {event_type}"
                )));
            }
        };
        Ok(event)
    }
}
