//! RTMP Message の Property-Based Testing
//!
//! RTMP 仕様に基づいた RTMP Message のエンコード/デコード roundtrip テストを行う。
//!
//! 参照: RTMP Specification 1.0
//! - RTMP 仕様 5.4 節: Protocol Control メッセージ (種別 1-6)
//! - RTMP 仕様 6.2 節: User Control メッセージ (種別 4)
//! - RTMP 仕様 7.1 節: メッセージの種別

use std::ops::RangeBounds;

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::{AmfValue, AmfVersion, Pair};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::amf3::Amf3Value;
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
};
use shiguredo_rtmp::rtmp_chunk::{RtmpChunkSize, RtmpChunkStreamId};
use shiguredo_rtmp::rtmp_command::TransactionId;
use shiguredo_rtmp::rtmp_message::{
    RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId, SetPeerBandwidthLimitType,
};
use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;
use shiguredo_rtmp::rtmp_message_encoder::RtmpMessageEncoder;
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
use shiguredo_rtmp::rtmp_user_control_event::RtmpUserControlEvent;

#[path = "../../tests/helpers/rtmp_ignored_message.rs"]
pub mod rtmp_ignored_message;

/// このファイルの roundtrip テストの PBT ケース数
const CASES: usize = 500;

/// 境界値テストの PBT ケース数
const BOUNDARY_CASES: usize = 100;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// AMF のキーに使う英字の文字プール
const ALPHABETIC_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// AMF の文字列値に使う英数字の文字プール
const ALPHANUMERIC_CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

// =============================================================================
// サンプラー定義
// =============================================================================

/// 指定した文字プールから、指定した長さ範囲の文字列を生成するサンプラー
fn sample_string_from_pool<R: RangeBounds<usize>>(
    ctx: &mut TestCaseContext,
    pool: &[u8],
    len_range: R,
) -> String {
    let len = noprop::sample_usize_in(ctx, len_range);
    (0..len)
        .map(|_| noprop::sample_choice(ctx, pool) as char)
        .collect()
}

/// 有限かつ正規の `f64` を生成するサンプラー
///
/// NaN・無限大・非正規化数・ゼロを除外し、AMF の数値として安全に往復できる値のみを返す。
fn sample_normal_f64(ctx: &mut TestCaseContext) -> f64 {
    noprop::sample_with_rejection(ctx, 8, |ctx| {
        let value = noprop::sample_f64(ctx);
        value.is_normal().then_some(value)
    })
}

/// 任意のメッセージヘッダーを生成するサンプラー
///
/// RTMP 仕様: タイムスタンプは 32-bit (最大 0xFFFFFFFF ミリ秒)
fn sample_message_header(ctx: &mut TestCaseContext) -> RtmpMessageHeader {
    let stream_id = noprop::sample_u32(ctx);
    let timestamp_ms = noprop::sample_u32(ctx);
    RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::new(stream_id),
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    }
}

/// Protocol Control Message 用のヘッダーを生成するサンプラー
///
/// RTMP 仕様 Section 5.4: Protocol Control Messages は message stream ID 0 を使用する
fn sample_pcm_header(ctx: &mut TestCaseContext) -> RtmpMessageHeader {
    let timestamp_ms = noprop::sample_u32(ctx);
    RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::PCM,
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    }
}

/// チャンクサイズを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.1: チャンクサイズは最小 1 バイト、この実装の最大は 65536 バイト
fn sample_chunk_size(ctx: &mut TestCaseContext) -> RtmpChunkSize {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1]) {
        // 境界値
        0 => RtmpChunkSize::new(1).expect("1 は有効なチャンクサイズ"),
        1 => RtmpChunkSize::new(128).expect("128 は有効なチャンクサイズ"),
        2 => RtmpChunkSize::new(65536).expect("65536 は有効なチャンクサイズ"),
        // 一般的な値
        _ => noprop::sample_with_rejection(ctx, 8, |ctx| {
            RtmpChunkSize::new(noprop::sample_usize_in(ctx, 1..=65536))
        }),
    }
}

/// チャンクストリーム ID を生成するサンプラー
///
/// RTMP 仕様 Section 5.3.1.1: 有効範囲は 2-65599
fn sample_chunk_stream_id(ctx: &mut TestCaseContext) -> RtmpChunkStreamId {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1, 1]) {
        // 境界値
        0 => RtmpChunkStreamId::new(2).expect("2 は有効なチャンクストリーム ID"),
        1 => RtmpChunkStreamId::new(63).expect("63 は有効なチャンクストリーム ID"),
        2 => RtmpChunkStreamId::new(64).expect("64 は有効なチャンクストリーム ID"),
        3 => RtmpChunkStreamId::new(319).expect("319 は有効なチャンクストリーム ID"),
        4 => RtmpChunkStreamId::new(320).expect("320 は有効なチャンクストリーム ID"),
        5 => RtmpChunkStreamId::new(65599).expect("65599 は有効なチャンクストリーム ID"),
        // 一般的な値
        _ => noprop::sample_with_rejection(ctx, 8, |ctx| {
            RtmpChunkStreamId::new(noprop::sample_u32_in(ctx, 2..=65599))
        }),
    }
}

/// SetPeerBandwidthLimitType を生成するサンプラー
///
/// RTMP 仕様 Section 5.4.5: Hard (0), Soft (1), Dynamic (2)
fn sample_limit_type(ctx: &mut TestCaseContext) -> SetPeerBandwidthLimitType {
    noprop::sample_choice(
        ctx,
        &[
            SetPeerBandwidthLimitType::Hard,
            SetPeerBandwidthLimitType::Soft,
            SetPeerBandwidthLimitType::Dynamic,
        ],
    )
}

/// RtmpUserControlEvent を生成するサンプラー
///
/// RTMP 仕様 6.2 節: User Control Message Events
fn sample_user_control_event(ctx: &mut TestCaseContext) -> RtmpUserControlEvent {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1, 1, 1, 1]) {
        // StreamBegin (イベント種別 0)
        0 => RtmpUserControlEvent::StreamBegin {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        // StreamEof (イベント種別 1)
        1 => RtmpUserControlEvent::StreamEof {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        // StreamDry (イベント種別 2)
        2 => RtmpUserControlEvent::StreamDry {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        // SetBufferLength (イベント種別 3)
        3 => RtmpUserControlEvent::SetBufferLength {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
            length: noprop::sample_u32(ctx),
        },
        // StreamIsRecorded (イベント種別 4)
        4 => RtmpUserControlEvent::StreamIsRecorded {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        // PingRequest (イベント種別 6)
        5 => RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        // PingResponse (イベント種別 7)
        6 => RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        // BufferEmpty (イベント種別 31)
        7 => RtmpUserControlEvent::BufferEmpty {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        // BufferReady (イベント種別 32)
        _ => RtmpUserControlEvent::BufferReady {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
    }
}

/// AudioFormat を生成するサンプラー
fn sample_audio_format(ctx: &mut TestCaseContext) -> AudioFormat {
    noprop::sample_choice(
        ctx,
        &[
            AudioFormat::LinearPcmPlatformEndian,
            AudioFormat::Adpcm,
            AudioFormat::Mp3,
            AudioFormat::LinearPcmLittleEndian,
            AudioFormat::Nellymoser16khzMono,
            AudioFormat::Nellymoser8KhzMono,
            AudioFormat::Nellymoser,
            AudioFormat::G711AlawLogarithmicPcm,
            AudioFormat::G711MuLawLogarithmicPcm,
            AudioFormat::Aac,
            AudioFormat::Speex,
            AudioFormat::Mp3_8khz,
            AudioFormat::DeviceSpecificSound,
        ],
    )
}

/// AudioSampleRate を生成するサンプラー
fn sample_audio_sample_rate(ctx: &mut TestCaseContext) -> AudioSampleRate {
    noprop::sample_choice(
        ctx,
        &[
            AudioSampleRate::Khz5,
            AudioSampleRate::Khz11,
            AudioSampleRate::Khz22,
            AudioSampleRate::Khz44,
        ],
    )
}

/// AudioFrame を生成するサンプラー
///
/// 注意: is_aac_sequence_header は format == Aac の場合のみ有効
fn sample_audio_frame(ctx: &mut TestCaseContext) -> AudioFrame {
    let timestamp_ms = noprop::sample_u32(ctx);
    let format = sample_audio_format(ctx);
    let sample_rate = sample_audio_sample_rate(ctx);
    // Speex は SoundSize 1 にそろうため、可逆性は正規化済みの値で検証する。
    let is_8bit_sample = noprop::sample_bool(ctx) && format != AudioFormat::Speex;
    let is_stereo = noprop::sample_bool(ctx);
    let is_aac_sequence_header = noprop::sample_bool(ctx);
    let data_len = noprop::sample_usize_in(ctx, 0..256);
    let data = noprop::sample_bytes_vec(ctx, data_len);

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
}

/// VideoCodec を生成するサンプラー
fn sample_video_codec(ctx: &mut TestCaseContext) -> VideoCodec {
    noprop::sample_choice(
        ctx,
        &[
            VideoCodec::Jpeg,
            VideoCodec::H263,
            VideoCodec::ScreenVideo,
            VideoCodec::Vp6,
            VideoCodec::Vp6WithAlpha,
            VideoCodec::ScreenVideoV2,
            VideoCodec::Avc,
        ],
    )
}

/// VideoFrameType を生成するサンプラー
fn sample_video_frame_type(ctx: &mut TestCaseContext) -> VideoFrameType {
    noprop::sample_choice(
        ctx,
        &[
            VideoFrameType::KeyFrame,
            VideoFrameType::InterFrame,
            VideoFrameType::DisposableInterFrame,
            VideoFrameType::GeneratedKeyFrame,
            VideoFrameType::VideoInfoOrCommandFrame,
        ],
    )
}

/// AvcPacketType を生成するサンプラー
fn sample_avc_packet_type(ctx: &mut TestCaseContext) -> AvcPacketType {
    noprop::sample_choice(
        ctx,
        &[
            AvcPacketType::SequenceHeader,
            AvcPacketType::NalUnit,
            AvcPacketType::EndOfSequence,
        ],
    )
}

/// ワイヤ形式で表現できるフィールドを持つ VideoFrame を生成する
fn sample_video_frame(ctx: &mut TestCaseContext) -> VideoFrame {
    let timestamp_ms = noprop::sample_u32(ctx);
    let frame_type = sample_video_frame_type(ctx);
    let codec = sample_video_codec(ctx);
    let data_len = noprop::sample_usize_in(ctx, 0..256);
    let data = noprop::sample_bytes_vec(ctx, data_len);

    // Adobe FLV 仕様 10.1 §E.4.3.1 では CodecID だけで AVC 追加ヘッダーの有無を決める。
    // フレームタイプによる例外が仕様に追加された場合は、生成条件も見直す。
    let (avc_packet_type, composition_timestamp_offset) = if codec == VideoCodec::Avc {
        let composition_offset = noprop::sample_i32_in(ctx, -8_388_608..=8_388_607);
        (
            Some(sample_avc_packet_type(ctx)),
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
}

/// AMF0 の簡単な値を生成するサンプラー (コマンド/データメッセージ用)
fn sample_simple_amf0_value(ctx: &mut TestCaseContext) -> Amf0Value {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1]) {
        // 数値
        0 => Amf0Value::Number(sample_normal_f64(ctx)),
        1 => Amf0Value::Number(0.0),
        // 真偽値
        2 => Amf0Value::Boolean(noprop::sample_bool(ctx)),
        // 文字列 (短め)
        3 => Amf0Value::String(sample_string_from_pool(ctx, ALPHANUMERIC_CHARS, 0..=50)),
        // Null と Undefined
        4 => Amf0Value::Null,
        _ => Amf0Value::Undefined,
    }
}

/// AMF3 の簡単な値を生成するサンプラー (コマンド/データメッセージ用)
fn sample_simple_amf3_value(ctx: &mut TestCaseContext) -> Amf3Value {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1, 1]) {
        // 整数 (i29 範囲)
        0 => Amf3Value::Integer(noprop::sample_i32_in(ctx, -268435456..=268435455)),
        // Double 型
        1 => Amf3Value::Double(sample_normal_f64(ctx)),
        2 => Amf3Value::Double(0.0),
        // 真偽値
        3 => Amf3Value::Boolean(noprop::sample_bool(ctx)),
        // 文字列 (短め)
        4 => Amf3Value::String(sample_string_from_pool(ctx, ALPHANUMERIC_CHARS, 0..=50)),
        // Null と Undefined
        5 => Amf3Value::Null,
        _ => Amf3Value::Undefined,
    }
}

/// AMF0 Object を生成するサンプラー
fn sample_amf0_object(ctx: &mut TestCaseContext) -> Amf0Value {
    let len = noprop::sample_usize_in(ctx, 0..5);
    let entries = (0..len)
        .map(|_| Pair {
            key: sample_string_from_pool(ctx, ALPHABETIC_CHARS, 1..=20),
            value: sample_simple_amf0_value(ctx),
        })
        .collect();
    Amf0Value::Object {
        class_name: None,
        entries,
    }
}

/// AMF3 Object を生成するサンプラー
fn sample_amf3_object(ctx: &mut TestCaseContext) -> Amf3Value {
    let len = noprop::sample_usize_in(ctx, 0..5);
    let entries = (0..len)
        .map(|_| Pair {
            key: sample_string_from_pool(ctx, ALPHABETIC_CHARS, 1..=20),
            value: sample_simple_amf3_value(ctx),
        })
        .collect();
    Amf3Value::Object {
        class_name: None,
        sealed_count: 0,
        entries,
    }
}

/// AmfValue (AMF0) を生成するサンプラー
fn sample_amf_value_amf0(ctx: &mut TestCaseContext) -> AmfValue {
    match noprop::sample_weighted_index(ctx, &[1, 1]) {
        0 => AmfValue::Amf0(sample_simple_amf0_value(ctx)),
        _ => AmfValue::Amf0(sample_amf0_object(ctx)),
    }
}

/// AmfValue (AMF3) を生成するサンプラー
fn sample_amf_value_amf3(ctx: &mut TestCaseContext) -> AmfValue {
    match noprop::sample_weighted_index(ctx, &[1, 1]) {
        0 => AmfValue::Amf3(sample_simple_amf3_value(ctx)),
        _ => AmfValue::Amf3(sample_amf3_object(ctx)),
    }
}

// =============================================================================
// Protocol Control メッセージ (RTMP 仕様 5.4 節)
// =============================================================================

/// SetChunkSize メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.1: Set Chunk Size (1)
fn sample_set_chunk_size_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let size = sample_chunk_size(ctx);
    RtmpMessage::SetChunkSize { header, size }
}

/// Abort メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.2: Abort Message (2)
fn sample_abort_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let chunk_stream_id = sample_chunk_stream_id(ctx);
    RtmpMessage::Abort {
        header,
        chunk_stream_id,
    }
}

/// Acknowledgement メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.3: Acknowledgement (3)
fn sample_ack_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let sequence_number = noprop::sample_u32(ctx);
    RtmpMessage::Ack {
        header,
        sequence_number,
    }
}

/// Window Acknowledgement Size メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.4: Window Acknowledgement Size (5)
fn sample_win_ack_size_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let size = noprop::sample_u32(ctx);
    RtmpMessage::WinAckSize { header, size }
}

/// Set Peer Bandwidth メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 5.4.5: Set Peer Bandwidth (6)
fn sample_set_peer_bandwidth_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let size = noprop::sample_u32(ctx);
    let limit_type = sample_limit_type(ctx);
    RtmpMessage::SetPeerBandwidth {
        header,
        size,
        limit_type,
    }
}

/// User Control メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 6.2: User Control Messages (4)
fn sample_user_control_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_pcm_header(ctx);
    let event = sample_user_control_event(ctx);
    RtmpMessage::UserControl { header, event }
}

// =============================================================================
// メディアメッセージ (RTMP 仕様 7.1.4 節、7.1.5 節)
// =============================================================================

/// Audio メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 7.1.4: Audio Message (8)
/// 注意: frame.timestamp は decode 時に header.timestamp から設定されるため、一致させる必要がある
fn sample_audio_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    let mut frame = sample_audio_frame(ctx);
    // frame.timestamp は header.timestamp と一致させる (decode 時の挙動に合わせる)
    frame.timestamp = header.timestamp;
    RtmpMessage::Audio { header, frame }
}

/// Video メッセージを生成するサンプラー
///
/// RTMP 仕様 Section 7.1.5: Video Message (9)
/// 注意: frame.timestamp は decode 時に header.timestamp から設定されるため、一致させる必要がある
fn sample_video_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    let mut frame = sample_video_frame(ctx);
    // frame.timestamp は header.timestamp と一致させる (decode 時の挙動に合わせる)
    frame.timestamp = header.timestamp;
    RtmpMessage::Video { header, frame }
}

// =============================================================================
// コマンド / データメッセージ (RTMP 仕様 7.1.1 節、7.1.2 節)
// =============================================================================

/// Command メッセージ (AMF0) を生成するサンプラー
///
/// RTMP 仕様 Section 7.1.1: Command Message (20)
fn sample_command_amf0_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    // コマンド名
    let name = sample_string_from_pool(ctx, ALPHABETIC_CHARS, 1..=20);
    // トランザクション ID
    let transaction_id = noprop::sample_u32_in(ctx, 0..=1_000_000);
    // コマンドオブジェクト
    let object = sample_amf0_object(ctx);
    // 任意の引数
    let args_len = noprop::sample_usize_in(ctx, 0..3);
    let args = (0..args_len).map(|_| sample_amf_value_amf0(ctx)).collect();
    RtmpMessage::Command {
        header,
        amf_version: AmfVersion::Amf0,
        name,
        transaction_id: TransactionId::from_f64(f64::from(transaction_id)),
        object: AmfValue::Amf0(object),
        args,
    }
}

/// Command メッセージ (AMF3) を生成するサンプラー
///
/// RTMP 仕様 Section 7.1.1: Command Message (17)
fn sample_command_amf3_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    // コマンド名
    let name = sample_string_from_pool(ctx, ALPHABETIC_CHARS, 1..=20);
    // トランザクション ID
    let transaction_id = noprop::sample_u32_in(ctx, 0..=1_000_000);
    // コマンドオブジェクト
    let object = sample_amf3_object(ctx);
    // 任意の引数
    let args_len = noprop::sample_usize_in(ctx, 0..3);
    let args = (0..args_len).map(|_| sample_amf_value_amf3(ctx)).collect();
    RtmpMessage::Command {
        header,
        amf_version: AmfVersion::Amf3,
        name,
        transaction_id: TransactionId::from_f64(f64::from(transaction_id)),
        object: AmfValue::Amf3(object),
        args,
    }
}

/// Data メッセージ (AMF0) を生成するサンプラー
///
/// RTMP 仕様 Section 7.1.2: Data Message (18)
fn sample_data_amf0_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    let values_len = noprop::sample_usize_in(ctx, 1..5);
    let values = (0..values_len)
        .map(|_| sample_amf_value_amf0(ctx))
        .collect();
    RtmpMessage::Data {
        header,
        amf_version: AmfVersion::Amf0,
        values,
    }
}

/// Data メッセージ (AMF3) を生成するサンプラー
///
/// RTMP 仕様 Section 7.1.2: Data Message (15)
fn sample_data_amf3_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    let header = sample_message_header(ctx);
    let values_len = noprop::sample_usize_in(ctx, 1..5);
    let values = (0..values_len)
        .map(|_| sample_amf_value_amf3(ctx))
        .collect();
    RtmpMessage::Data {
        header,
        amf_version: AmfVersion::Amf3,
        values,
    }
}

// =============================================================================
// 全 RtmpMessage のサンプラー
// =============================================================================

/// すべての種類の RtmpMessage を生成するサンプラー
fn sample_rtmp_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]) {
        // Protocol Control メッセージ
        0 => sample_set_chunk_size_message(ctx),
        1 => sample_abort_message(ctx),
        2 => sample_ack_message(ctx),
        3 => sample_win_ack_size_message(ctx),
        4 => sample_set_peer_bandwidth_message(ctx),
        5 => sample_user_control_message(ctx),
        // メディアメッセージ
        6 => sample_audio_message(ctx),
        7 => sample_video_message(ctx),
        // コマンド / データメッセージ
        8 => sample_command_amf0_message(ctx),
        9 => sample_command_amf3_message(ctx),
        10 => sample_data_amf0_message(ctx),
        _ => sample_data_amf3_message(ctx),
    }
}

// =============================================================================
// Roundtrip テスト
// =============================================================================

/// Protocol Control Message: SetChunkSize の roundtrip を検証する
#[test]
fn set_chunk_size_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_set_chunk_size_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// Protocol Control Message: Abort の roundtrip を検証する
#[test]
fn abort_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_abort_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// Protocol Control Message: Ack の roundtrip を検証する
#[test]
fn ack_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_ack_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// Protocol Control Message: WinAckSize の roundtrip を検証する
#[test]
fn win_ack_size_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_win_ack_size_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// Protocol Control Message: SetPeerBandwidth の roundtrip を検証する
#[test]
fn set_peer_bandwidth_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_set_peer_bandwidth_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// User Control Message の roundtrip を検証する
#[test]
fn user_control_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_user_control_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// 完成済みの短い User Control と、分割して受信する正しい User Control を区別する
#[test]
fn user_control_decode_distinguishes_truncated_payload_from_partial_input() -> noprop::TestResult {
    use shiguredo_rtmp::error::ErrorKind;
    use shiguredo_rtmp::rtmp_chunk::RtmpChunk;
    use shiguredo_rtmp::rtmp_chunk_encoder::RtmpChunkEncoder;
    use shiguredo_rtmp::rtmp_message::RtmpMessageType;

    let header = RtmpMessageHeader::PCM;
    let chunk_stream_id = RtmpChunkStreamId::new(2).expect("2 は制御用のチャンクストリーム ID");
    // Adobe RTMP 仕様 6.2 節は単一チャンクへの収容を推奨する。ここでは受信側の組み立てを
    // 検査するため、4 バイトのチャンクサイズを通知し、イベントを複数チャンクに分ける。
    let chunk_size = RtmpChunkSize::new(4).expect("4 バイトは有効なチャンクサイズ");
    let set_chunk_size = RtmpMessage::SetChunkSize {
        header,
        size: chunk_size,
    };
    let mut setup_wire = Vec::new();
    RtmpMessageEncoder::default()
        .encode(&mut setup_wire, chunk_stream_id, set_chunk_size.clone())
        .expect("チャンクサイズの通知をエンコードできるはず");

    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
        let event = RtmpUserControlEvent::SetBufferLength {
            stream_id: RtmpMessageStreamId::MEDIA,
            length: noprop::sample_u32(ctx),
        };
        let mut payload = Vec::new();
        event.encode(&mut payload);
        let short_len = noprop::sample_usize_in(ctx, (chunk_size.get() + 1)..payload.len());

        for (payload, expected) in [
            (payload[..short_len].to_vec(), Err(ErrorKind::InvalidData)),
            // 宣言長が 0 の場合はヘッダーだけで完成する。
            (Vec::new(), Err(ErrorKind::InvalidData)),
            (
                payload,
                Ok(Some(RtmpMessage::UserControl { header, event })),
            ),
        ] {
            // メッセージの宣言長は実際のペイロード長と一致させ、不足はイベント内部だけに作る。
            let mut encoder = RtmpChunkEncoder::default();
            encoder.set_chunk_size(chunk_size);
            let mut wire = Vec::new();
            encoder.encode(
                &mut wire,
                &RtmpChunk {
                    chunk_stream_id,
                    message_stream_id: header.stream_id,
                    message_type: RtmpMessageType::UserControl,
                    timestamp: header.timestamp,
                    payload,
                },
            );
            let mut decoder = RtmpMessageDecoder::default();
            decoder.feed_buf(&setup_wire);
            assert_eq!(
                decoder
                    .decode()
                    .expect("チャンクサイズの通知を読み取れるはず"),
                Some(set_chunk_size.clone()),
                "イベント受信前にチャンクサイズの通知が受理されるはず"
            );

            // 入力を 1 バイトずつ渡し、チャンクの途中と、チャンクの境目の両方を通す。
            let (last, prefix) = wire.split_last().expect("チャンクにはヘッダーがあるはず");
            for byte in prefix {
                decoder.feed_buf(core::slice::from_ref(byte));
                assert_eq!(
                    decoder.decode().map_err(|e| e.kind),
                    Ok(None),
                    "メッセージの宣言長に達するまでは追加受信を待つはず"
                );
            }

            // 最後のバイトでメッセージが揃い、初めてイベントデータの妥当性が決まる。
            decoder.feed_buf(core::slice::from_ref(last));
            assert_eq!(
                decoder.decode().map_err(|e| e.kind),
                expected,
                "完成したメッセージではイベントのデコード結果が返るはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// Audio Message の roundtrip を検証する
#[test]
fn audio_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        roundtrip_test(sample_audio_message(ctx));
        Ok(())
    })?;
    Ok(())
}

/// Video Message の roundtrip を検証する
#[test]
fn video_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_video_message(ctx);
        roundtrip_test(message.clone());

        // 生成したメッセージでも、AVC のオフセットの両端は毎回往復できることを確認する。
        let RtmpMessage::Video { header, mut frame } = message else {
            panic!("映像メッセージを生成するはず");
        };
        frame.frame_type = VideoFrameType::InterFrame;
        frame.codec = VideoCodec::Avc;
        frame.avc_packet_type = Some(AvcPacketType::NalUnit);
        for offset in [-8_388_608, 8_388_607] {
            frame.composition_timestamp_offset = RtmpTimestampDelta::from_millis(offset);
            roundtrip_test(RtmpMessage::Video {
                header,
                frame: frame.clone(),
            });
        }
        Ok(())
    })?;
    Ok(())
}

/// Command Message (AMF0) の roundtrip を検証する
#[test]
fn command_amf0_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_command_amf0_message(ctx);
        // NaN を含む場合は比較不可能なため、このケースをスキップする
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// Command Message (AMF3) の roundtrip を検証する
#[test]
fn command_amf3_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_command_amf3_message(ctx);
        // NaN を含む場合は比較不可能なため、このケースをスキップする
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// Data Message (AMF0) の roundtrip を検証する
#[test]
fn data_amf0_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_data_amf0_message(ctx);
        // NaN を含む場合は比較不可能なため、このケースをスキップする
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// Data Message (AMF3) の roundtrip を検証する
#[test]
fn data_amf3_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_data_amf3_message(ctx);
        // NaN を含む場合は比較不可能なため、このケースをスキップする
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// 全種類の RtmpMessage の roundtrip を検証する
#[test]
fn all_message_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let message = sample_rtmp_message(ctx);
        // NaN を含む場合は比較不可能なため、このケースをスキップする
        if contains_nan(&message) {
            return Ok(());
        }
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

mod ignored_message_reception {
    use noprop::TestCaseContext;
    use shiguredo_rtmp::error::ErrorKind;
    use shiguredo_rtmp::rtmp_chunk::{RtmpChunk, RtmpChunkSize, RtmpChunkStreamId};
    use shiguredo_rtmp::rtmp_chunk_encoder::RtmpChunkEncoder;
    use shiguredo_rtmp::rtmp_connection::RtmpMessageChannel;
    use shiguredo_rtmp::rtmp_message::{
        RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId, RtmpMessageType,
    };
    use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;
    use shiguredo_rtmp::rtmp_message_encoder::RtmpMessageEncoder;
    use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;

    use super::rtmp_ignored_message::sample_ignored_messages;
    use super::{BOUNDARY_CASES, SEED_ENV, sample_audio_message};

    /// 無視対象が完成しても decode は途切れず、すでに受信した対応済みメッセージを返す
    #[test]
    fn decode_skips_ignored_message_without_requesting_more_input() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
            for sample in sample_ignored_messages(ctx) {
                let mut decoder = RtmpMessageDecoder::default();
                decoder.feed_buf(&sample.incomplete);
                assert_eq!(
                    decoder.decode().expect("途中の入力はエラーにならないはず"),
                    None,
                    "未完成のメッセージは取り出せないはず"
                );

                decoder.feed_buf(&sample.completion);
                // 先に完成した音声の後、連続する無視対象を飛ばして形式 1 の音声を返す。
                for expected in sample.into_audio_messages() {
                    assert_eq!(
                        decoder.decode().expect("完成した入力をデコードできるはず"),
                        Some(expected),
                        "対応済みメッセージを追加の入力なしで完成順に取り出せるはず"
                    );
                }
                assert_eq!(
                    decoder
                        .decode()
                        .expect("全メッセージ消費後にエラーにならないはず"),
                    None,
                    "無視対象は通常メッセージとして返さないはず"
                );
            }
            Ok(())
        })?;
        Ok(())
    }

    /// 受信途中の無視対象を Abort で破棄し、同じチャンクストリームで新しいメッセージを受信する
    #[test]
    fn abort_discards_an_incomplete_ignored_message() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
            let chunk_stream_id = RtmpChunkStreamId::new(noprop::sample_u32_in(ctx, 3..=63))
                .expect("3 から 63 は有効なチャンクストリーム ID");
            let chunk_size = RtmpChunkSize::default().get();
            let payload_len = noprop::sample_usize_in(ctx, (chunk_size + 1)..=(2 * chunk_size));
            let mut wire = Vec::new();
            RtmpChunkEncoder::default().encode(
                &mut wire,
                &RtmpChunk {
                    chunk_stream_id,
                    message_stream_id: RtmpMessageStreamId::MEDIA,
                    message_type: RtmpMessageType::SharedObjectAmf0,
                    timestamp: RtmpTimestamp::ZERO,
                    payload: noprop::sample_bytes_vec(ctx, payload_len),
                },
            );
            // Adobe RTMP 仕様（2012 年 12 月版）5.3.1 節のこの形式ではヘッダーは計 12 バイト。
            // 5.4.2 節に従い 1 チャンク目で送信を打ち切って Abort を送る。形式が変わる場合は見直す。
            wire.truncate(12 + chunk_size);
            let abort = RtmpMessage::Abort {
                header: RtmpMessageHeader::PCM,
                chunk_stream_id,
            };
            let control_id = RtmpChunkStreamId::new(2).expect("2 は制御用のチャンクストリーム ID");
            RtmpMessageEncoder::default()
                .encode(&mut wire, control_id, abort.clone())
                .expect("制御ストリームに Abort を書き込めるはず");
            let audio = sample_audio_message(ctx);
            // 新しいエンコーダーを使い、破棄したメッセージの圧縮履歴を参照しない。
            RtmpMessageEncoder::default()
                .encode(&mut wire, chunk_stream_id, audio.clone())
                .expect("同じチャンクストリームに新しい音声を書き込めるはず");

            let mut decoder = RtmpMessageDecoder::default();
            decoder.feed_buf(&wire);
            for message in [Some(abort), Some(audio), None] {
                assert_eq!(
                    decoder
                        .decode()
                        .expect("Abort 後の有効な入力を受理するはず"),
                    message,
                    "中断したデータを破棄し、Abort と新しい音声だけを返すはず"
                );
            }
            Ok(())
        })?;
        Ok(())
    }

    /// 対応外の予約・未定義 ID では従来と同じエラーになる
    #[test]
    fn unrecognized_message_type_is_invalid_data() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
            let type_id = match noprop::sample_usize_in(ctx, 0..5) {
                0 => 0,
                1 => 7,
                2 => noprop::sample_u8_in(ctx, 10..=14),
                3 => 21,
                _ => noprop::sample_u8_in(ctx, 23..=u8::MAX),
            };
            // Adobe RTMP 仕様（2012 年 12 月版）5.3.1 節の形式 0 のヘッダーを使う。
            // 公開エンコーダーに渡せない未知型を検証するため、型 ID 以外を固定する。
            // CSID 3、タイムスタンプ 0、ペイロード長 0、メッセージストリーム ID 1 とする。
            // ヘッダー形式や型 ID の割り当てが変わる場合は入力を見直す。
            let buf = [3, 0, 0, 0, 0, 0, 0, type_id, 1, 0, 0, 0];
            let mut decoder = RtmpMessageDecoder::default();
            decoder.feed_buf(&buf);
            let error = decoder
                .decode()
                .expect_err("未知型はデコードを拒否するはず");
            assert_eq!(
                error.kind,
                ErrorKind::InvalidData,
                "未知型 {type_id} のデコードエラー種別"
            );

            let error = RtmpMessageChannel::default()
                .feed_recv_buf(&buf)
                .expect_err("チャネルは未知型のエラーを呼び出し側へ返すはず");
            assert_eq!(
                error.kind,
                ErrorKind::InvalidData,
                "未知型 {type_id} のチャネル受信エラー種別"
            );
            Ok(())
        })?;
        Ok(())
    }
}

// =============================================================================
// 境界値テスト
// =============================================================================

/// SetChunkSize の境界値を roundtrip で検証する
///
/// RTMP 仕様 Section 5.4.1: 最小 1 バイト、この実装の最大は 65536 バイト
#[test]
fn set_chunk_size_boundary() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
        let size = noprop::sample_choice(ctx, &[1usize, 128, 65536]);
        let chunk_size = RtmpChunkSize::new(size).expect("境界値はすべて有効なチャンクサイズ");
        let message = RtmpMessage::SetChunkSize {
            header: RtmpMessageHeader::PCM,
            size: chunk_size,
        };
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// Abort メッセージの境界値を roundtrip で検証する
///
/// chunk stream ID の境界値 (2, 63, 64, 319, 320, 65599)
#[test]
fn abort_boundary() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
        let id = noprop::sample_choice(ctx, &[2u32, 63, 64, 319, 320, 65599]);
        let chunk_stream_id =
            RtmpChunkStreamId::new(id).expect("境界値はすべて有効なチャンクストリーム ID");
        let message = RtmpMessage::Abort {
            header: RtmpMessageHeader::PCM,
            chunk_stream_id,
        };
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

/// タイムスタンプの境界値を roundtrip で検証する
///
/// RTMP 仕様: 拡張タイムスタンプは 0xFFFFFF 以上で使用する
#[test]
fn timestamp_boundary() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(BOUNDARY_CASES, |ctx: &mut TestCaseContext| {
        let timestamp_ms = noprop::sample_choice(
            ctx,
            // 0 / 拡張タイムスタンプ直前 / 拡張タイムスタンプ境界 / 拡張タイムスタンプ / 最大値
            &[0u32, 0xFFFFFE, 0xFFFFFF, 0x1000000, 0xFFFFFFFF],
        );
        let message = RtmpMessage::Ack {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::PCM,
                timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            },
            sequence_number: 12345,
        };
        roundtrip_test(message);
        Ok(())
    })?;
    Ok(())
}

// =============================================================================
// ヘルパー関数
// =============================================================================

/// encode → decode の roundtrip テストを行う
///
/// # パニック
///
/// エンコード済みメッセージのデコードに失敗した場合、または roundtrip の結果が
/// 元のメッセージと一致しない場合にパニックする。
fn roundtrip_test(message: RtmpMessage) {
    let chunk_stream_id = RtmpChunkStreamId::new(3).expect("3 は有効なチャンクストリーム ID");
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();

    encoder
        .encode(&mut buf, chunk_stream_id, message.clone())
        .expect("生成した RTMP メッセージはエンコードできるはず");

    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&buf);
    let decoded = decoder
        .decode()
        .expect("エンコード済みメッセージのデコードは失敗しない")
        .expect("エンコード済みメッセージが 1 つ存在する");

    assert_eq!(
        decoded, message,
        "encode/decode の roundtrip 結果が一致しない"
    );
}

/// RtmpMessage が NaN を含むかどうかをチェックする
fn contains_nan(message: &RtmpMessage) -> bool {
    match message {
        RtmpMessage::Command { object, args, .. } => {
            contains_nan_amf(object) || args.iter().any(contains_nan_amf)
        }
        RtmpMessage::Data { values, .. } => values.iter().any(contains_nan_amf),
        _ => false,
    }
}

/// AmfValue が NaN を含むかどうかをチェックする
fn contains_nan_amf(value: &AmfValue) -> bool {
    match value {
        AmfValue::Amf0(v) => contains_nan_amf0(v),
        AmfValue::Amf3(v) => contains_nan_amf3(v),
    }
}

/// Amf0Value が NaN を含むかどうかをチェックする
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

/// Amf3Value が NaN を含むかどうかをチェックする
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
