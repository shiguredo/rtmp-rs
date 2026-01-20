//! RTMP Message の Property-Based Testing
//!
//! RTMP 仕様に基づいた RTMP Message のエンコード/デコード roundtrip テストを行う。
//!
//! 参照: rtmp_specification_1.0.md
//! - Section 5.4: Protocol Control Messages (Type 1-6)
//! - Section 6.2: User Control Messages (Type 4)
//! - Section 7.1: Types of Messages

use proptest::collection::vec;
use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    Amf0Value, Amf3Value, AmfValue, AmfVersion, Pair, RtmpChunkSize, RtmpChunkStreamId,
    RtmpMessage, RtmpMessageDecoder, RtmpMessageEncoder, RtmpMessageHeader, RtmpMessageStreamId,
    RtmpTimestamp, RtmpTimestampDelta, RtmpUserControlEvent, SetPeerBandwidthLimitType,
    TransactionId,
};
use shiguredo_rtmp::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
};

// =============================================================================
// Strategy 定義
// =============================================================================

/// RtmpMessageHeader を生成する Strategy
fn arb_message_header() -> impl Strategy<Value = RtmpMessageHeader> {
    // RTMP 仕様: タイムスタンプは 32-bit (最大 0xFFFFFFFF ミリ秒)
    (any::<u32>(), any::<u32>()).prop_map(|(stream_id, timestamp_ms)| RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::new(stream_id),
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    })
}

/// Protocol Control Message 用のヘッダを生成する Strategy
/// RTMP 仕様 Section 5.4: Protocol Control Messages は message stream ID 0 を使用
fn arb_pcm_header() -> impl Strategy<Value = RtmpMessageHeader> {
    any::<u32>().prop_map(|timestamp_ms| RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::PCM,
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    })
}

/// RtmpChunkSize を生成する Strategy
/// RTMP 仕様 Section 5.4.1: チャンクサイズは最小 1 バイト、推奨最小 128 バイト
fn arb_chunk_size() -> impl Strategy<Value = RtmpChunkSize> {
    prop_oneof![
        // 境界値
        Just(RtmpChunkSize::new(1).unwrap()),
        Just(RtmpChunkSize::new(128).unwrap()),   // デフォルト
        Just(RtmpChunkSize::new(65536).unwrap()), // MAX
        // 一般的な値
        (1usize..=65536).prop_filter_map("valid chunk size", RtmpChunkSize::new),
    ]
}

/// RtmpChunkStreamId を生成する Strategy
/// RTMP 仕様 Section 5.3.1.1: 有効範囲は 2-65599
fn arb_chunk_stream_id() -> impl Strategy<Value = RtmpChunkStreamId> {
    prop_oneof![
        // 境界値
        Just(RtmpChunkStreamId::new(2).unwrap()),
        Just(RtmpChunkStreamId::new(63).unwrap()),
        Just(RtmpChunkStreamId::new(64).unwrap()),
        Just(RtmpChunkStreamId::new(319).unwrap()),
        Just(RtmpChunkStreamId::new(320).unwrap()),
        Just(RtmpChunkStreamId::new(65599).unwrap()),
        // 一般的な値
        (2u32..=65599).prop_filter_map("valid chunk stream id", RtmpChunkStreamId::new),
    ]
}

/// SetPeerBandwidthLimitType を生成する Strategy
/// RTMP 仕様 Section 5.4.5: Hard (0), Soft (1), Dynamic (2)
fn arb_limit_type() -> impl Strategy<Value = SetPeerBandwidthLimitType> {
    prop_oneof![
        Just(SetPeerBandwidthLimitType::Hard),
        Just(SetPeerBandwidthLimitType::Soft),
        Just(SetPeerBandwidthLimitType::Dynamic),
    ]
}

/// RtmpUserControlEvent を生成する Strategy
/// RTMP 仕様 Section 6.2: User Control Message Events
fn arb_user_control_event() -> impl Strategy<Value = RtmpUserControlEvent> {
    prop_oneof![
        // StreamBegin (0)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamBegin {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        // StreamEof (1)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamEof {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        // StreamDry (2)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamDry {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        // SetBufferLength (3)
        (any::<u32>(), any::<u32>()).prop_map(|(stream_id, length)| {
            RtmpUserControlEvent::SetBufferLength {
                stream_id: RtmpMessageStreamId::new(stream_id),
                length,
            }
        }),
        // StreamIsRecorded (4)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamIsRecorded {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        // PingRequest (6)
        any::<u32>().prop_map(|ms| RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(ms)
        }),
        // PingResponse (7)
        any::<u32>().prop_map(|ms| RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(ms)
        }),
        // BufferEmpty (31)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::BufferEmpty {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        // BufferReady (32)
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::BufferReady {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
    ]
}

/// AudioFormat を生成する Strategy
fn arb_audio_format() -> impl Strategy<Value = AudioFormat> {
    prop_oneof![
        Just(AudioFormat::Adpcm),
        Just(AudioFormat::Mp3),
        Just(AudioFormat::LinearPcmLittleEndian),
        Just(AudioFormat::Nellymoser16khzMono),
        Just(AudioFormat::Nellymoser8KhzMono),
        Just(AudioFormat::Nellymoser),
        Just(AudioFormat::G711AlawLogarithmicPcm),
        Just(AudioFormat::G711MuLawLogarithmicPcm),
        Just(AudioFormat::Aac),
        Just(AudioFormat::Speex),
        Just(AudioFormat::Mp3_8khz),
        Just(AudioFormat::DeviceSpecificSound),
    ]
}

/// AudioSampleRate を生成する Strategy
fn arb_audio_sample_rate() -> impl Strategy<Value = AudioSampleRate> {
    prop_oneof![
        Just(AudioSampleRate::Khz5),
        Just(AudioSampleRate::Khz11),
        Just(AudioSampleRate::Khz22),
        Just(AudioSampleRate::Khz44),
    ]
}

/// AudioFrame を生成する Strategy
/// 注意: is_aac_sequence_header は format == Aac の場合のみ有効
fn arb_audio_frame() -> impl Strategy<Value = AudioFrame> {
    (
        any::<u32>(),
        arb_audio_format(),
        arb_audio_sample_rate(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        vec(any::<u8>(), 0..256),
    )
        .prop_map(
            |(
                timestamp_ms,
                format,
                sample_rate,
                is_8bit_sample,
                is_stereo,
                is_aac_sequence_header,
                data,
            )| {
                // is_aac_sequence_header は AAC 以外では常に false
                let is_aac_sequence_header = if format == AudioFormat::Aac {
                    is_aac_sequence_header
                } else {
                    false
                };
                AudioFrame {
                    timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                    format,
                    sample_rate,
                    is_8bit_sample,
                    is_stereo,
                    is_aac_sequence_header,
                    data,
                }
            },
        )
}

/// VideoCodec を生成する Strategy
fn arb_video_codec() -> impl Strategy<Value = VideoCodec> {
    prop_oneof![
        Just(VideoCodec::Jpeg),
        Just(VideoCodec::H263),
        Just(VideoCodec::ScreenVideo),
        Just(VideoCodec::Vp6),
        Just(VideoCodec::Vp6WithAlpha),
        Just(VideoCodec::ScreenVideoV2),
        Just(VideoCodec::Avc),
    ]
}

/// VideoFrameType を生成する Strategy
fn arb_video_frame_type() -> impl Strategy<Value = VideoFrameType> {
    prop_oneof![
        Just(VideoFrameType::KeyFrame),
        Just(VideoFrameType::InterFrame),
        Just(VideoFrameType::DisposableInterFrame),
        Just(VideoFrameType::GeneratedKeyFrame),
        Just(VideoFrameType::VideoInfoOrCommandFrame),
    ]
}

/// VideoFrame を生成する Strategy
/// 注意:
/// - avc_packet_type は codec == Avc かつ frame_type != VideoInfoOrCommandFrame の場合のみ有効 (かつ必須)
/// - 非 Avc コーデック または VideoInfoOrCommandFrame では avc_packet_type は None
/// - composition_timestamp_offset は avc_packet_type が Some の場合のみエンコードされる
/// - composition_timestamp_offset は i24 範囲 (-8388608 ~ 8388607 ms) である必要がある
fn arb_video_frame() -> impl Strategy<Value = VideoFrame> {
    (
        any::<u32>(),
        -8388608i32..=8388607i32, // composition offset (i24 範囲)
        arb_video_frame_type(),
        arb_video_codec(),
        prop_oneof![
            Just(AvcPacketType::SequenceHeader),
            Just(AvcPacketType::NalUnit),
            Just(AvcPacketType::EndOfSequence),
        ],
        vec(any::<u8>(), 0..256),
    )
        .prop_map(
            |(timestamp_ms, composition_offset, frame_type, codec, avc_type, data)| {
                // AVC コーデックかつ VideoInfoOrCommandFrame でない場合のみ avc_packet_type を設定
                // FLV 仕様: VideoInfoOrCommandFrame は AVC でも avc_packet_type を持たない
                let (avc_packet_type, composition_timestamp_offset) = if codec == VideoCodec::Avc
                    && frame_type != VideoFrameType::VideoInfoOrCommandFrame
                {
                    (
                        Some(avc_type),
                        RtmpTimestampDelta::from_millis(composition_offset),
                    )
                } else {
                    (None, RtmpTimestampDelta::ZERO)
                };
                VideoFrame {
                    timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                    composition_timestamp_offset,
                    frame_type,
                    codec,
                    avc_packet_type,
                    data,
                }
            },
        )
}

/// AMF0 の簡単な値を生成する Strategy (コマンド/データメッセージ用)
fn arb_simple_amf0_value() -> impl Strategy<Value = Amf0Value> {
    prop_oneof![
        // 数値
        prop::num::f64::NORMAL.prop_map(Amf0Value::Number),
        Just(Amf0Value::Number(0.0)),
        // 真偽値
        any::<bool>().prop_map(Amf0Value::Boolean),
        // 文字列 (短め)
        "[a-zA-Z0-9]{0,50}".prop_map(Amf0Value::String),
        // Null / Undefined
        Just(Amf0Value::Null),
        Just(Amf0Value::Undefined),
    ]
}

/// AMF3 の簡単な値を生成する Strategy (コマンド/データメッセージ用)
fn arb_simple_amf3_value() -> impl Strategy<Value = Amf3Value> {
    prop_oneof![
        // 整数 (i29 範囲)
        (-268435456i32..=268435455i32).prop_map(Amf3Value::Integer),
        // Double
        prop::num::f64::NORMAL.prop_map(Amf3Value::Double),
        Just(Amf3Value::Double(0.0)),
        // 真偽値
        any::<bool>().prop_map(Amf3Value::Boolean),
        // 文字列 (短め)
        "[a-zA-Z0-9]{0,50}".prop_map(Amf3Value::String),
        // Null / Undefined
        Just(Amf3Value::Null),
        Just(Amf3Value::Undefined),
    ]
}

/// AMF0 Object を生成する Strategy
fn arb_amf0_object() -> impl Strategy<Value = Amf0Value> {
    vec(("[a-zA-Z]{1,20}", arb_simple_amf0_value()), 0..5).prop_map(|entries| Amf0Value::Object {
        class_name: None,
        entries: entries
            .into_iter()
            .map(|(key, value)| Pair { key, value })
            .collect(),
    })
}

/// AMF3 Object を生成する Strategy
fn arb_amf3_object() -> impl Strategy<Value = Amf3Value> {
    vec(("[a-zA-Z]{1,20}", arb_simple_amf3_value()), 0..5).prop_map(|entries| {
        let entries: Vec<_> = entries
            .into_iter()
            .map(|(key, value)| Pair { key, value })
            .collect();
        Amf3Value::Object {
            class_name: None,
            sealed_count: 0,
            entries,
        }
    })
}

/// AmfValue (AMF0) を生成する Strategy
fn arb_amf_value_amf0() -> impl Strategy<Value = AmfValue> {
    prop_oneof![
        arb_simple_amf0_value().prop_map(AmfValue::Amf0),
        arb_amf0_object().prop_map(AmfValue::Amf0),
    ]
}

/// AmfValue (AMF3) を生成する Strategy
fn arb_amf_value_amf3() -> impl Strategy<Value = AmfValue> {
    prop_oneof![
        arb_simple_amf3_value().prop_map(AmfValue::Amf3),
        arb_amf3_object().prop_map(AmfValue::Amf3),
    ]
}

// =============================================================================
// Protocol Control Messages (Section 5.4)
// =============================================================================

/// SetChunkSize メッセージを生成する Strategy
/// RTMP 仕様 Section 5.4.1: Set Chunk Size (1)
fn arb_set_chunk_size_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), arb_chunk_size())
        .prop_map(|(header, size)| RtmpMessage::SetChunkSize { header, size })
}

/// Abort メッセージを生成する Strategy
/// RTMP 仕様 Section 5.4.2: Abort Message (2)
fn arb_abort_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), arb_chunk_stream_id()).prop_map(|(header, chunk_stream_id)| {
        RtmpMessage::Abort {
            header,
            chunk_stream_id,
        }
    })
}

/// Acknowledgement メッセージを生成する Strategy
/// RTMP 仕様 Section 5.4.3: Acknowledgement (3)
fn arb_ack_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), any::<u32>()).prop_map(|(header, sequence_number)| RtmpMessage::Ack {
        header,
        sequence_number,
    })
}

/// Window Acknowledgement Size メッセージを生成する Strategy
/// RTMP 仕様 Section 5.4.4: Window Acknowledgement Size (5)
fn arb_win_ack_size_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), any::<u32>())
        .prop_map(|(header, size)| RtmpMessage::WinAckSize { header, size })
}

/// Set Peer Bandwidth メッセージを生成する Strategy
/// RTMP 仕様 Section 5.4.5: Set Peer Bandwidth (6)
fn arb_set_peer_bandwidth_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), any::<u32>(), arb_limit_type()).prop_map(|(header, size, limit_type)| {
        RtmpMessage::SetPeerBandwidth {
            header,
            size,
            limit_type,
        }
    })
}

/// User Control メッセージを生成する Strategy
/// RTMP 仕様 Section 6.2: User Control Messages (4)
fn arb_user_control_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_pcm_header(), arb_user_control_event())
        .prop_map(|(header, event)| RtmpMessage::UserControl { header, event })
}

// =============================================================================
// Media Messages (Section 7.1.4, 7.1.5)
// =============================================================================

/// Audio メッセージを生成する Strategy
/// RTMP 仕様 Section 7.1.4: Audio Message (8)
/// 注意: frame.timestamp は decode 時に header.timestamp から設定されるため、一致させる必要がある
fn arb_audio_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_message_header(), arb_audio_frame()).prop_map(|(header, mut frame)| {
        // frame.timestamp は header.timestamp と一致させる (decode 時の挙動に合わせる)
        frame.timestamp = header.timestamp;
        RtmpMessage::Audio { header, frame }
    })
}

/// Video/Audio メッセージ用のヘッダを生成する Strategy
fn arb_media_header() -> impl Strategy<Value = RtmpMessageHeader> {
    (any::<u32>(), any::<u32>()).prop_map(|(stream_id, timestamp_ms)| RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::new(stream_id),
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    })
}

/// Video メッセージを生成する Strategy
/// RTMP 仕様 Section 7.1.5: Video Message (9)
/// 注意: frame.timestamp は decode 時に header.timestamp から設定されるため、一致させる必要がある
fn arb_video_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_media_header(), arb_video_frame()).prop_map(|(header, mut frame)| {
        // frame.timestamp は header.timestamp と一致させる (decode 時の挙動に合わせる)
        frame.timestamp = header.timestamp;
        if frame.avc_packet_type.is_none() {
            frame.composition_timestamp_offset = RtmpTimestampDelta::ZERO;
        }
        RtmpMessage::Video { header, frame }
    })
}

// =============================================================================
// Command/Data Messages (Section 7.1.1, 7.1.2)
// =============================================================================

/// Command メッセージ (AMF0) を生成する Strategy
/// RTMP 仕様 Section 7.1.1: Command Message (20)
fn arb_command_amf0_message() -> impl Strategy<Value = RtmpMessage> {
    (
        arb_message_header(),
        "[a-zA-Z]{1,20}",                // command name
        0i64..=1_000_000i64,             // transaction id
        arb_amf0_object(),               // command object
        vec(arb_amf_value_amf0(), 0..3), // optional args
    )
        .prop_map(
            |(header, name, transaction_id, object, args)| RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name,
                transaction_id: TransactionId::from_f64(transaction_id as f64),
                object: AmfValue::Amf0(object),
                args,
            },
        )
}

/// Command メッセージ (AMF3) を生成する Strategy
/// RTMP 仕様 Section 7.1.1: Command Message (17)
fn arb_command_amf3_message() -> impl Strategy<Value = RtmpMessage> {
    (
        arb_message_header(),
        "[a-zA-Z]{1,20}",                // command name
        0i64..=1_000_000i64,             // transaction id
        arb_amf3_object(),               // command object
        vec(arb_amf_value_amf3(), 0..3), // optional args
    )
        .prop_map(
            |(header, name, transaction_id, object, args)| RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf3,
                name,
                transaction_id: TransactionId::from_f64(transaction_id as f64),
                object: AmfValue::Amf3(object),
                args,
            },
        )
}

/// Data メッセージ (AMF0) を生成する Strategy
/// RTMP 仕様 Section 7.1.2: Data Message (18)
fn arb_data_amf0_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_message_header(), vec(arb_amf_value_amf0(), 1..5)).prop_map(|(header, values)| {
        RtmpMessage::Data {
            header,
            amf_version: AmfVersion::Amf0,
            values,
        }
    })
}

/// Data メッセージ (AMF3) を生成する Strategy
/// RTMP 仕様 Section 7.1.2: Data Message (15)
fn arb_data_amf3_message() -> impl Strategy<Value = RtmpMessage> {
    (arb_message_header(), vec(arb_amf_value_amf3(), 1..5)).prop_map(|(header, values)| {
        RtmpMessage::Data {
            header,
            amf_version: AmfVersion::Amf3,
            values,
        }
    })
}

// =============================================================================
// 全 RtmpMessage の Strategy
// =============================================================================

/// すべての種類の RtmpMessage を生成する Strategy
fn arb_rtmp_message() -> impl Strategy<Value = RtmpMessage> {
    prop_oneof![
        // Protocol Control Messages
        arb_set_chunk_size_message(),
        arb_abort_message(),
        arb_ack_message(),
        arb_win_ack_size_message(),
        arb_set_peer_bandwidth_message(),
        arb_user_control_message(),
        // Media Messages
        arb_audio_message(),
        arb_video_message(),
        // Command/Data Messages
        arb_command_amf0_message(),
        arb_command_amf3_message(),
        arb_data_amf0_message(),
        arb_data_amf3_message(),
    ]
}

// =============================================================================
// Roundtrip テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Protocol Control Message: SetChunkSize の roundtrip
    #[test]
    fn set_chunk_size_roundtrip(message in arb_set_chunk_size_message()) {
        roundtrip_test(message)?;
    }

    /// Protocol Control Message: Abort の roundtrip
    #[test]
    fn abort_roundtrip(message in arb_abort_message()) {
        roundtrip_test(message)?;
    }

    /// Protocol Control Message: Ack の roundtrip
    #[test]
    fn ack_roundtrip(message in arb_ack_message()) {
        roundtrip_test(message)?;
    }

    /// Protocol Control Message: WinAckSize の roundtrip
    #[test]
    fn win_ack_size_roundtrip(message in arb_win_ack_size_message()) {
        roundtrip_test(message)?;
    }

    /// Protocol Control Message: SetPeerBandwidth の roundtrip
    #[test]
    fn set_peer_bandwidth_roundtrip(message in arb_set_peer_bandwidth_message()) {
        roundtrip_test(message)?;
    }

    /// User Control Message の roundtrip
    #[test]
    fn user_control_roundtrip(message in arb_user_control_message()) {
        roundtrip_test(message)?;
    }

    /// Audio Message の roundtrip
    #[test]
    fn audio_roundtrip(message in arb_audio_message()) {
        roundtrip_test(message)?;
    }

    /// Video Message の roundtrip
    #[test]
    fn video_roundtrip(message in arb_video_message()) {
        roundtrip_test(message)?;
    }

    /// Command Message (AMF0) の roundtrip
    #[test]
    fn command_amf0_roundtrip(message in arb_command_amf0_message()) {
        // NaN を含む場合はスキップ (比較不可能)
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message)?;
    }

    /// Command Message (AMF3) の roundtrip
    #[test]
    fn command_amf3_roundtrip(message in arb_command_amf3_message()) {
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message)?;
    }

    /// Data Message (AMF0) の roundtrip
    #[test]
    fn data_amf0_roundtrip(message in arb_data_amf0_message()) {
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message)?;
    }

    /// Data Message (AMF3) の roundtrip
    #[test]
    fn data_amf3_roundtrip(message in arb_data_amf3_message()) {
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message)?;
    }

    /// 全種類の RtmpMessage の roundtrip
    #[test]
    fn all_message_roundtrip(message in arb_rtmp_message()) {
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message)?;
    }
}

// =============================================================================
// 境界値テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// SetChunkSize の境界値テスト
    /// RTMP 仕様 Section 5.4.1: 最小 1 バイト、最大 0x7FFFFFFF (31-bit)
    #[test]
    fn set_chunk_size_boundary(
        size in prop_oneof![
            Just(1usize),       // 最小
            Just(128usize),     // デフォルト
            Just(65536usize),   // MAX (実装の制限)
        ]
    ) {
        if let Some(chunk_size) = RtmpChunkSize::new(size) {
            let message = RtmpMessage::SetChunkSize {
                header: RtmpMessageHeader {
                    stream_id: RtmpMessageStreamId::PCM,
                    timestamp: RtmpTimestamp::ZERO,
                },
                size: chunk_size,
            };
            roundtrip_test(message)?;
        }
    }

    /// Abort メッセージの境界値テスト
    /// chunk stream ID の境界値 (2, 63, 64, 319, 320, 65599)
    #[test]
    fn abort_boundary(
        id in prop_oneof![
            Just(2u32),
            Just(63u32),
            Just(64u32),
            Just(319u32),
            Just(320u32),
            Just(65599u32),
        ]
    ) {
        if let Some(chunk_stream_id) = RtmpChunkStreamId::new(id) {
            let message = RtmpMessage::Abort {
                header: RtmpMessageHeader {
                    stream_id: RtmpMessageStreamId::PCM,
                    timestamp: RtmpTimestamp::ZERO,
                },
                chunk_stream_id,
            };
            roundtrip_test(message)?;
        }
    }

    /// タイムスタンプ境界値テスト
    /// RTMP 仕様: 拡張タイムスタンプは 0xFFFFFF 以上で使用
    #[test]
    fn timestamp_boundary(
        timestamp_ms in prop_oneof![
            Just(0u32),
            Just(0xFFFFFEu32),   // 拡張タイムスタンプ直前
            Just(0xFFFFFFu32),   // 拡張タイムスタンプ境界
            Just(0x1000000u32),  // 拡張タイムスタンプ
            Just(0xFFFFFFFFu32), // 最大値
        ]
    ) {
        let message = RtmpMessage::Ack {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::PCM,
                timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            },
            sequence_number: 12345,
        };
        roundtrip_test(message)?;
    }
}

// =============================================================================
// ヘルパー関数
// =============================================================================

/// encode → decode の roundtrip テストを行う
fn roundtrip_test(message: RtmpMessage) -> Result<(), TestCaseError> {
    let chunk_stream_id = RtmpChunkStreamId::new(3).unwrap();
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();

    encoder.encode(&mut buf, chunk_stream_id, message.clone());

    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&buf);
    let decoded = decoder
        .decode()
        .map_err(|e| TestCaseError::fail(format!("decode failed: {e:?}")))?
        .ok_or_else(|| TestCaseError::fail("decoded message is None"))?;

    prop_assert_eq!(decoded, message, "roundtrip value mismatch");
    Ok(())
}

/// RtmpMessage が NaN を含むかどうかをチェック
fn contains_nan(message: &RtmpMessage) -> bool {
    match message {
        RtmpMessage::Command { object, args, .. } => {
            contains_nan_amf(object) || args.iter().any(contains_nan_amf)
        }
        RtmpMessage::Data { values, .. } => values.iter().any(contains_nan_amf),
        _ => false,
    }
}

fn contains_nan_amf(value: &AmfValue) -> bool {
    match value {
        AmfValue::Amf0(v) => contains_nan_amf0(v),
        AmfValue::Amf3(v) => contains_nan_amf3(v),
    }
}

fn contains_nan_amf0(value: &Amf0Value) -> bool {
    match value {
        Amf0Value::Number(n) => n.is_nan(),
        Amf0Value::Object { entries, .. } => entries.iter().any(|p| contains_nan_amf0(&p.value)),
        Amf0Value::EcmaArray { entries } => entries.iter().any(|p| contains_nan_amf0(&p.value)),
        Amf0Value::Array { entries } => entries.iter().any(contains_nan_amf0),
        Amf0Value::AvmPlus(v) => contains_nan_amf3(v),
        _ => false,
    }
}

fn contains_nan_amf3(value: &Amf3Value) -> bool {
    match value {
        Amf3Value::Double(n) => n.is_nan(),
        Amf3Value::Array {
            assoc_entries,
            dense_entries,
        } => {
            assoc_entries.iter().any(|p| contains_nan_amf3(&p.value))
                || dense_entries.iter().any(contains_nan_amf3)
        }
        Amf3Value::Object { entries, .. } => entries.iter().any(|p| contains_nan_amf3(&p.value)),
        Amf3Value::DoubleVector { entries, .. } => entries.iter().any(|n| n.is_nan()),
        Amf3Value::ObjectVector { entries, .. } => entries.iter().any(contains_nan_amf3),
        Amf3Value::Dictionary { entries, .. } => entries
            .iter()
            .any(|p| contains_nan_amf3(&p.key) || contains_nan_amf3(&p.value)),
        _ => false,
    }
}
