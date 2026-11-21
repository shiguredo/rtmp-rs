//! RTMP のメッセージと、そのヘッダー・メッセージ種別を定義するモジュール

use alloc::string::String;
use alloc::vec::Vec;

use crate::amf::{AmfValue, AmfVersion};
use crate::error::Error;
use crate::media::{AudioFrame, VideoFrame};
use crate::rtmp_chunk::{RtmpChunkSize, RtmpChunkStreamId};
use crate::rtmp_command::TransactionId;
use crate::rtmp_timestamp::RtmpTimestamp;
use crate::rtmp_user_control_event::RtmpUserControlEvent;

/// RTMP メッセージを伝送するメッセージストリームの ID
///
/// RTMP 仕様ではメッセージストリーム ID は 32 ビットの値であり、
/// プロトコル制御メッセージはメッセージストリーム ID 0 の制御ストリームで送信する
/// （RTMP 仕様 5.4 節）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RtmpMessageStreamId(u32);

impl RtmpMessageStreamId {
    /// プロトコル制御メッセージ用のストリーム ID（0）
    ///
    /// プロトコル制御メッセージはメッセージストリーム ID 0 の制御ストリームで
    /// 送信することが RTMP 仕様 5.4 節で規定されている。
    pub const PCM: Self = Self(0);

    /// 接続直後にサーバーが最初に発行するストリームの ID（1）
    ///
    /// このストリームの用途は定まっていないが、値自体は何でもよいため
    /// 1 で固定している。
    pub const FIRST: Self = Self(1);

    /// サーバーがメディアストリーム用に発行する ID（2）
    ///
    /// このクレート では 1 つの接続で複数のストリームを扱わないため、
    /// 固定値を使っている。
    pub const MEDIA: Self = Self(2);

    /// 指定した値のメッセージストリーム ID を生成する
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// メッセージストリーム ID の値（32 ビット）を返す
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// RTMP メッセージの種別
///
/// variant の値は RTMP 仕様で定義されたメッセージタイプ ID に対応する。
/// メッセージタイプ ID の一覧は RTMP 仕様 6 章を参照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtmpMessageType {
    /// チャンクサイズを変更するプロトコル制御メッセージ（タイプ ID 1）
    ///
    /// RTMP 仕様 5.4.1 節で定義されている。
    SetChunkSize = 1,

    /// 受信中のメッセージの破棄を指示するプロトコル制御メッセージ（タイプ ID 2）
    ///
    /// RTMP 仕様 5.4.2 節で定義されている。
    Abort = 2,

    /// 累積受信バイト数を相手に通知するプロトコル制御メッセージ（タイプ ID 3）
    ///
    /// RTMP 仕様 5.4.3 節で定義されている。
    Ack = 3,

    /// ユーザー制御メッセージ（タイプ ID 4）
    ///
    /// ストリームの開始や Ping などのイベントを伝える。
    UserControl = 4,

    /// ACK を送信するウィンドウサイズを通知するプロトコル制御メッセージ（タイプ ID 5）
    ///
    /// RTMP 仕様 5.4.4 節で定義されている。
    WinAckSize = 5,

    /// 相手の出力帯域幅を制限するプロトコル制御メッセージ（タイプ ID 6）
    ///
    /// RTMP 仕様 5.4.5 節で定義されている。
    SetPeerBandwidth = 6,

    /// 音声メッセージ（タイプ ID 8）
    Audio = 8,

    /// 映像メッセージ（タイプ ID 9）
    Video = 9,

    /// AMF3 でエンコードされたデータメッセージ（タイプ ID 15）
    DataAmf3 = 15,

    /// AMF3 でエンコードされたコマンドメッセージ（タイプ ID 17）
    CommandAmf3 = 17,

    /// AMF0 でエンコードされたデータメッセージ（タイプ ID 18）
    DataAmf0 = 18,

    /// AMF0 でエンコードされたコマンドメッセージ（タイプ ID 20）
    CommandAmf0 = 20,
}

impl RtmpMessageType {
    /// メッセージタイプ ID からメッセージ種別を生成する
    ///
    /// 未知のタイプ ID の場合は [`Error`] を返す。
    pub fn from_type_id(type_id: u8) -> Result<Self, Error> {
        match type_id {
            1 => Ok(RtmpMessageType::SetChunkSize),
            2 => Ok(RtmpMessageType::Abort),
            3 => Ok(RtmpMessageType::Ack),
            4 => Ok(RtmpMessageType::UserControl),
            5 => Ok(RtmpMessageType::WinAckSize),
            6 => Ok(RtmpMessageType::SetPeerBandwidth),
            8 => Ok(RtmpMessageType::Audio),
            9 => Ok(RtmpMessageType::Video),
            15 => Ok(RtmpMessageType::DataAmf3),
            17 => Ok(RtmpMessageType::CommandAmf3),
            18 => Ok(RtmpMessageType::DataAmf0),
            20 => Ok(RtmpMessageType::CommandAmf0),
            _ => Err(Error::invalid_data(format!(
                "unknown message type: {type_id}"
            ))),
        }
    }
}

/// RTMP メッセージの共通ヘッダー
///
/// メッセージストリーム ID とタイムスタンプを保持する。
/// メッセージヘッダーはチャンクストリーム上ではチャンクメッセージヘッダーとして
/// 伝送され、デコード後のメッセージではこの構造体として扱われる
/// （Enhanced RTMP 仕様の「RTMP Message Format」の節を参照）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtmpMessageHeader {
    /// メッセージストリーム ID
    pub stream_id: RtmpMessageStreamId,

    /// メッセージのタイムスタンプ（ミリ秒単位）
    pub timestamp: RtmpTimestamp,
}

impl RtmpMessageHeader {
    /// 制御ストリーム（メッセージストリーム ID 0）向けのヘッダー
    ///
    /// タイムスタンプは 0 である。
    pub const PCM: Self = Self {
        stream_id: RtmpMessageStreamId::PCM,
        timestamp: RtmpTimestamp::ZERO,
    };
}

/// RTMP メッセージ
///
/// メッセージ種別ごとのヘッダーとペイロードを variant で表現する。
#[derive(Debug, Clone, PartialEq)]
pub enum RtmpMessage {
    /// チャンクサイズを変更するプロトコル制御メッセージ
    SetChunkSize {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 変更後のチャンクサイズ
        size: RtmpChunkSize,
    },

    /// 受信中のメッセージの破棄を指示するプロトコル制御メッセージ
    Abort {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 破棄対象のメッセージが伝送されているチャンクストリームの ID
        chunk_stream_id: RtmpChunkStreamId,
    },

    /// 累積受信バイト数を相手に通知するプロトコル制御メッセージ
    Ack {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 累積受信バイト数
        ///
        /// RTMP 仕様ではシーケンス番号と呼ばれるが、実際には受信側が
        /// これまでに受信した累積バイト数を表す。
        sequence_number: u32,
    },

    /// ACK を送信するウィンドウサイズを通知するプロトコル制御メッセージ
    WinAckSize {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// ウィンドウサイズ（バイト単位）
        size: u32,
    },

    /// 相手の出力帯域幅を制限するプロトコル制御メッセージ
    SetPeerBandwidth {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 帯域幅（バイト単位）
        size: u32,

        /// 帯域幅の制限方法
        limit_type: SetPeerBandwidthLimitType,
    },

    /// ユーザー制御メッセージ
    UserControl {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 通知するイベント
        event: RtmpUserControlEvent,
    },

    /// 音声メッセージ
    Audio {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 音声フレーム
        frame: AudioFrame,
    },

    /// 映像メッセージ
    Video {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// 映像フレーム
        frame: VideoFrame,
    },

    /// コマンドメッセージ
    Command {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// AMF のバージョン（AMF0 または AMF3）
        amf_version: AmfVersion,

        /// コマンド名（`connect` や `publish` など）
        name: String,

        /// トランザクション ID
        transaction_id: TransactionId,

        /// コマンドの対象となるオブジェクト
        object: AmfValue,

        /// コマンドに付随する追加の引数
        args: Vec<AmfValue>,
    },

    /// データメッセージ
    Data {
        /// このメッセージのヘッダー
        header: RtmpMessageHeader,

        /// AMF のバージョン（AMF0 または AMF3）
        amf_version: AmfVersion,

        /// データとして伝送する AMF の値の列
        values: Vec<AmfValue>,
    },
}

impl RtmpMessage {
    /// このメッセージの共通ヘッダーを返す
    pub fn header(&self) -> RtmpMessageHeader {
        match self {
            RtmpMessage::SetChunkSize { header, .. }
            | RtmpMessage::Abort { header, .. }
            | RtmpMessage::Ack { header, .. }
            | RtmpMessage::WinAckSize { header, .. }
            | RtmpMessage::SetPeerBandwidth { header, .. }
            | RtmpMessage::UserControl { header, .. }
            | RtmpMessage::Audio { header, .. }
            | RtmpMessage::Video { header, .. }
            | RtmpMessage::Command { header, .. }
            | RtmpMessage::Data { header, .. } => *header,
        }
    }

    /// このメッセージの種別を返す
    pub fn message_type(&self) -> RtmpMessageType {
        match self {
            RtmpMessage::SetChunkSize { .. } => RtmpMessageType::SetChunkSize,
            RtmpMessage::Abort { .. } => RtmpMessageType::Abort,
            RtmpMessage::Ack { .. } => RtmpMessageType::Ack,
            RtmpMessage::WinAckSize { .. } => RtmpMessageType::WinAckSize,
            RtmpMessage::SetPeerBandwidth { .. } => RtmpMessageType::SetPeerBandwidth,
            RtmpMessage::UserControl { .. } => RtmpMessageType::UserControl,
            RtmpMessage::Audio { .. } => RtmpMessageType::Audio,
            RtmpMessage::Video { .. } => RtmpMessageType::Video,
            RtmpMessage::Command { amf_version, .. } => match amf_version {
                AmfVersion::Amf0 => RtmpMessageType::CommandAmf0,
                AmfVersion::Amf3 => RtmpMessageType::CommandAmf3,
            },
            RtmpMessage::Data { amf_version, .. } => match amf_version {
                AmfVersion::Amf0 => RtmpMessageType::DataAmf0,
                AmfVersion::Amf3 => RtmpMessageType::DataAmf3,
            },
        }
    }

    /// ストリームの開始を通知するユーザー制御メッセージを生成する
    ///
    /// `stream_id` で指定したストリームの開始を相手に伝える。
    /// ヘッダーは制御ストリーム（メッセージストリーム ID 0）を使う。
    pub fn stream_begin(stream_id: RtmpMessageStreamId) -> Self {
        Self::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::StreamBegin { stream_id },
        }
    }

    /// ACK の送信間隔を指定するウィンドウ承認サイズのメッセージを生成する
    ///
    /// `size` はバイト単位のウィンドウサイズである（RTMP 仕様 5.4.4 節）。
    /// ヘッダーは制御ストリーム（メッセージストリーム ID 0）を使う。
    pub fn win_ack_size(size: u32) -> Self {
        Self::WinAckSize {
            header: RtmpMessageHeader::PCM,
            size,
        }
    }

    /// 相手の出力帯域幅を制限するメッセージを生成する
    ///
    /// `size` はバイト単位の帯域幅である（RTMP 仕様 5.4.5 節）。
    /// 制限方法は [`SetPeerBandwidthLimitType::Hard`] 固定である。
    pub fn set_peer_bandwidth(size: u32) -> Self {
        Self::SetPeerBandwidth {
            header: RtmpMessageHeader::PCM,
            size,

            // いったん挙動が一番シンプルな Hard 固定にしておく
            limit_type: SetPeerBandwidthLimitType::Hard,
        }
    }

    /// 累積受信バイト数を通知する ACK メッセージを生成する
    ///
    /// `total_bytes_received` は受信側がこれまでに受信した累積バイト数である
    /// （RTMP 仕様 5.4.3 節）。
    /// ヘッダーは制御ストリーム（メッセージストリーム ID 0）を使う。
    pub fn ack(total_bytes_received: u32) -> Self {
        Self::Ack {
            header: RtmpMessageHeader::PCM,
            sequence_number: total_bytes_received,
        }
    }

    /// チャンクサイズを変更するメッセージを生成する
    ///
    /// このメッセージを送信した後のチャンクは、新しいチャンクサイズで
    /// エンコードされる（RTMP 仕様 5.4.1 節）。
    /// ヘッダーは制御ストリーム（メッセージストリーム ID 0）を使う。
    pub fn set_chunk_size(size: RtmpChunkSize) -> Self {
        Self::SetChunkSize {
            header: RtmpMessageHeader::PCM,
            size,
        }
    }
}

/// Set Peer Bandwidth メッセージで使う帯域幅の制限方法
///
/// 値は RTMP 仕様 5.4.5 節で定義されている Limit Type に対応する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetPeerBandwidthLimitType {
    /// 相手の出力帯域幅を通知した値に制限する（値 0）
    Hard = 0,

    /// 通知した値と現在有効な制限のうち小さい方に制限する（値 1）
    Soft,

    /// 直前の制限方法に応じて扱いを決める（値 2）
    Dynamic,
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::string::ToString;

    use crate::amf::Pair;
    use crate::amf0::Amf0Value;
    use crate::amf3::Amf3Value;
    use crate::rtmp_message_decoder::RtmpMessageDecoder;
    use crate::rtmp_message_encoder::RtmpMessageEncoder;
    use crate::rtmp_timestamp::RtmpTimestampDelta;
    use crate::{AudioFormat, AudioSampleRate};

    fn header(stream_id: u32, timestamp_ms: u32) -> RtmpMessageHeader {
        RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::new(stream_id),
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        }
    }

    fn pcm_header(timestamp_ms: u32) -> RtmpMessageHeader {
        RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::PCM,
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        }
    }

    fn encode_decode_roundtrip(message: RtmpMessage) -> RtmpMessage {
        let chunk_stream_id = RtmpChunkStreamId::new(3).expect("チャンクストリーム ID が有効");
        let mut encoder = RtmpMessageEncoder::default();
        let mut buf = Vec::new();

        encoder.encode(&mut buf, chunk_stream_id, message);

        let mut decoder = RtmpMessageDecoder::default();

        decoder.feed_buf(&buf);
        decoder
            .decode()
            .expect("エンコード済みメッセージのデコードは失敗しない")
            .expect("メッセージが存在する")
    }

    #[test]
    fn test_set_chunk_size_decode_encode() {
        let msg = RtmpMessage::SetChunkSize {
            header: pcm_header(0),
            size: RtmpChunkSize::new(1234).expect("チャンクサイズが有効"),
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_abort_decode_encode() {
        let msg = RtmpMessage::Abort {
            header: pcm_header(0),
            chunk_stream_id: RtmpChunkStreamId::new(10).expect("チャンクストリーム ID が有効"),
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_ack_decode_encode() {
        let msg = RtmpMessage::Ack {
            header: pcm_header(0),
            sequence_number: 56789,
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_win_ack_size_decode_encode() {
        let msg = RtmpMessage::WinAckSize {
            header: pcm_header(0),
            size: 45678,
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_set_peer_bandwidth_decode_encode() {
        let msg = RtmpMessage::SetPeerBandwidth {
            header: pcm_header(0),
            size: 4567,
            limit_type: SetPeerBandwidthLimitType::Soft,
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_stream_begin_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::StreamBegin {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_stream_eof_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::StreamEof {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_stream_dry_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::StreamDry {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_set_buffer_length_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::SetBufferLength {
                stream_id: RtmpMessageStreamId::new(10),
                length: 1234,
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_stream_is_recorded_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::StreamIsRecorded {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_ping_request_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::PingRequest {
                timestamp: RtmpTimestamp::from_millis(3456),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_ping_response_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::PingResponse {
                timestamp: RtmpTimestamp::from_millis(3456),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_buffer_empty_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::BufferEmpty {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_user_control_buffer_ready_decode_encode() {
        let msg = RtmpMessage::UserControl {
            header: pcm_header(0),
            event: RtmpUserControlEvent::BufferReady {
                stream_id: RtmpMessageStreamId::new(10),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_audio_decode_encode() {
        let msg = RtmpMessage::Audio {
            header: header(10, 1234),
            frame: AudioFrame {
                timestamp: RtmpTimestamp::from_millis(1234),
                format: AudioFormat::Aac,
                sample_rate: AudioSampleRate::Khz44,
                is_8bit_sample: true,
                is_stereo: true,
                is_aac_sequence_header: false,
                data: b"dummy_audio_data".to_vec(),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_video_decode_encode() {
        let msg = RtmpMessage::Video {
            header: header(10, 1234),
            frame: VideoFrame {
                timestamp: RtmpTimestamp::from_millis(1234),
                composition_timestamp_offset: RtmpTimestampDelta::from_millis(1234),
                frame_type: crate::media::VideoFrameType::KeyFrame,
                codec: crate::media::VideoCodec::Avc,
                avc_packet_type: Some(crate::media::AvcPacketType::NalUnit),
                data: b"dummy_video_data".to_vec(),
            },
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_command_amf0_decode_encode() {
        let msg = RtmpMessage::Command {
            header: header(10, 0),
            amf_version: AmfVersion::Amf0,
            name: "connect".to_string(),
            transaction_id: TransactionId::CONNECT,
            object: AmfValue::Amf0(Amf0Value::Object {
                class_name: None,
                entries: vec![Pair {
                    key: "a".to_string(),
                    value: Amf0Value::String("b".to_string()),
                }],
            }),
            args: vec![
                AmfValue::Amf0(Amf0Value::String("string".to_string())),
                AmfValue::Amf0(Amf0Value::Array {
                    entries: vec![
                        Amf0Value::Number(1.0),
                        Amf0Value::Number(2.0),
                        Amf0Value::Number(3.0),
                    ],
                }),
            ],
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_command_amf3_decode_encode() {
        let msg = RtmpMessage::Command {
            header: header(10, 0),
            amf_version: AmfVersion::Amf3,
            name: "connect".to_string(),
            transaction_id: TransactionId::CONNECT,
            object: AmfValue::Amf3(Amf3Value::Object {
                class_name: None,
                sealed_count: 1,
                entries: vec![Pair {
                    key: "a".to_string(),
                    value: Amf3Value::String("b".to_string()),
                }],
            }),
            args: vec![
                AmfValue::Amf3(Amf3Value::String("string".to_string())),
                AmfValue::Amf3(Amf3Value::Array {
                    assoc_entries: vec![],
                    dense_entries: vec![
                        Amf3Value::Double(1.0),
                        Amf3Value::Double(2.0),
                        Amf3Value::Double(3.0),
                    ],
                }),
            ],
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_data_amf0_decode_encode() {
        let msg = RtmpMessage::Data {
            header: header(10, 0),
            amf_version: AmfVersion::Amf0,
            values: vec![AmfValue::Amf0(Amf0Value::Object {
                class_name: None,
                entries: vec![
                    Pair {
                        key: "a".to_string(),
                        value: Amf0Value::String("b".to_string()),
                    },
                    Pair {
                        key: "c".to_string(),
                        value: Amf0Value::Number(10.4),
                    },
                ],
            })],
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_data_amf3_decode_encode() {
        let msg = RtmpMessage::Data {
            header: header(10, 0),
            amf_version: AmfVersion::Amf3,
            values: vec![AmfValue::Amf3(Amf3Value::Object {
                class_name: None,
                sealed_count: 2,
                entries: vec![
                    Pair {
                        key: "a".to_string(),
                        value: Amf3Value::String("b".to_string()),
                    },
                    Pair {
                        key: "c".to_string(),
                        value: Amf3Value::Double(10.4),
                    },
                ],
            })],
        };

        let decoded = encode_decode_roundtrip(msg.clone());
        assert_eq!(msg, decoded);
    }
}
