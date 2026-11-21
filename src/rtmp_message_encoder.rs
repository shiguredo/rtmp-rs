//! RTMP のメッセージをチャンクストリームへエンコードするエンコーダーを提供するモジュール

use alloc::vec::Vec;

use crate::amf::{AmfValue, AmfVersion};
use crate::bytes::BytesWriter;
use crate::rtmp_chunk::{RtmpChunk, RtmpChunkStreamId};
use crate::rtmp_chunk_encoder::RtmpChunkEncoder;
use crate::rtmp_command::TransactionId;
use crate::rtmp_message::RtmpMessage;

/// RTMP メッセージのエンコーダー
///
/// メッセージをチャンクに分割し、引数で渡されたバイト列の末尾へ書き出す。
/// チャンクストリームごとの送信状態を内部に保持するため、
/// 同じ接続では同じエンコーダーを使い続ける。
#[derive(Debug, Default)]
pub struct RtmpMessageEncoder {
    chunk_encoder: RtmpChunkEncoder,
}

impl RtmpMessageEncoder {
    /// メッセージをエンコードして `buf` の末尾に追加する
    ///
    /// `chunk_stream_id` はメッセージを伝送するチャンクストリームの ID である。
    /// メッセージをチャンクへ分割する際のチャンクサイズには、
    /// このエンコーダーが保持している値を使う。
    ///
    /// Set Chunk Size メッセージをエンコードした場合は、このメッセージ自身を
    /// 現在のチャンクサイズでエンコードした後に、続くメッセージへ新しい
    /// チャンクサイズを適用する（RTMP 仕様 5.4.1 節）。
    ///
    /// このクレート は Abort メッセージを送信しないため、Abort 送信に伴う
    /// チャンクストリームの状態リセットは行わない。デコード側は相手から
    /// Abort が届く可能性があるため対応している。
    pub fn encode(
        &mut self,
        buf: &mut Vec<u8>,
        chunk_stream_id: RtmpChunkStreamId,
        message: RtmpMessage,
    ) {
        let new_chunk_size = if let RtmpMessage::SetChunkSize { size, .. } = message {
            // [補足]
            // SetChunkSize はメッセージのチャンクへのエンコード方法自体に影響するので、
            // 呼び出し元ではなく、このメソッド内でハンドリングする必要がある
            //
            // なお、このクレート 自体は Abort を発行することがないので、
            // そのハンドリングは入っていない
            // （デコード側は、相手が送信してくる可能性があるので対応が必要）
            Some(size)
        } else {
            None
        };

        let chunk = self.message_to_chunk(chunk_stream_id, message);
        self.chunk_encoder.encode(buf, &chunk);

        if let Some(size) = new_chunk_size {
            self.chunk_encoder.set_chunk_size(size);
        }
    }

    fn message_to_chunk(
        &self,
        chunk_stream_id: RtmpChunkStreamId,
        message: RtmpMessage,
    ) -> RtmpChunk {
        let header = message.header();
        let message_type = message.message_type();
        let mut payload = Vec::new();
        self.encode_message_payload(&mut payload, message);
        RtmpChunk {
            chunk_stream_id,
            message_stream_id: header.stream_id,
            message_type,
            timestamp: header.timestamp,
            payload,
        }
    }

    fn encode_message_payload(&self, buf: &mut Vec<u8>, message: RtmpMessage) {
        match message {
            RtmpMessage::SetChunkSize { size, .. } => {
                buf.write_u32(size.get() as u32);
            }
            RtmpMessage::Abort {
                chunk_stream_id, ..
            } => {
                buf.write_u32(chunk_stream_id.get());
            }
            RtmpMessage::Ack {
                sequence_number, ..
            } => {
                buf.write_u32(sequence_number);
            }
            RtmpMessage::WinAckSize { size, .. } => {
                buf.write_u32(size);
            }
            RtmpMessage::SetPeerBandwidth {
                size, limit_type, ..
            } => {
                buf.write_u32(size);
                buf.write_u8(limit_type as u8);
            }
            RtmpMessage::Audio { frame, .. } => {
                crate::flv::encode_audio_frame(buf, &frame);
            }
            RtmpMessage::Video { frame, .. } => {
                crate::flv::encode_video_frame(buf, &frame);
            }
            RtmpMessage::Data { values, .. } => {
                self.encode_data_payload(buf, &values);
            }
            RtmpMessage::UserControl { event, .. } => {
                event.encode(buf);
            }
            RtmpMessage::Command {
                amf_version,
                name,
                transaction_id,
                object,
                args,
                ..
            } => {
                self.encode_command(buf, amf_version, &name, transaction_id, &object, &args);
            }
        }
    }

    fn encode_command(
        &self,
        buf: &mut Vec<u8>,
        amf_version: AmfVersion,
        name: &str,
        transaction_id: TransactionId,
        object: &AmfValue,
        args: &[AmfValue],
    ) {
        AmfValue::from((amf_version, name)).encode(buf);
        AmfValue::from((amf_version, transaction_id.get() as f64)).encode(buf);
        object.encode(buf);
        for arg in args {
            arg.encode(buf);
        }
    }

    fn encode_data_payload(&self, buf: &mut Vec<u8>, values: &[AmfValue]) {
        for value in values {
            value.encode(buf);
        }
    }
}
