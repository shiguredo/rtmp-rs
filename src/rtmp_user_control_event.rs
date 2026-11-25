//! RTMP のユーザーコントロールイベントをエンコード・デコードするモジュール
//! 標準のイベントの種類とデータ形式は Adobe RTMP 仕様 6.2 節と 7.1.7 節に従う。

use alloc::vec::Vec;

use crate::bytes::BytesReader;
use crate::error::{Error, ErrorKind};
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
/// （Adobe RTMP 仕様 6.2 節と 7.1.7 節）。
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
    /// イベントを識別する名前を返す
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
    /// `buf` を完成済みの 1 メッセージのペイロードとして扱い、先頭からイベントタイプと
    /// イベントデータを読み取る。イベントデータの後ろに余分なバイトがあっても無視する。
    ///
    /// # エラー
    ///
    /// イベントタイプが未知の場合、またはタイプや必要なイベントデータの長さが足りない場合は
    /// `kind` が [`ErrorKind::InvalidData`] のエラーを返す。
    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        Self::decode_payload(buf).map_err(|mut error| {
            // 完成済みのペイロードには追加受信がない。読み取りに失敗したソース位置を保ち、長さ不足を分類し直す。
            if error.kind == ErrorKind::InsufficientBuffer {
                error.kind = ErrorKind::InvalidData;
                error.reason = "truncated user control event payload".into();
            }
            error
        })
    }

    fn decode_payload(mut buf: &[u8]) -> Result<Self, Error> {
        // Adobe RTMP 仕様 6.2 節と 7.1.7 節が標準イベントのタイプと各フィールドの幅を定めている。
        // ペイロード形式を変更する場合はフィールドの読み取りとエラー条件を合わせて見直す。
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
