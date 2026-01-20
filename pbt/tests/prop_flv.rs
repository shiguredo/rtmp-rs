//! FLV (Audio/Video Frame) の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, RtmpTimestamp, RtmpTimestampDelta,
    VideoCodec, VideoFrame, VideoFrameType, decode_audio_frame, decode_video_frame,
    encode_audio_frame, encode_video_frame,
};

/// AudioFormat を生成する
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

/// AudioSampleRate を生成する
fn arb_audio_sample_rate() -> impl Strategy<Value = AudioSampleRate> {
    prop_oneof![
        Just(AudioSampleRate::Khz5),
        Just(AudioSampleRate::Khz11),
        Just(AudioSampleRate::Khz22),
        Just(AudioSampleRate::Khz44),
    ]
}

/// AudioFrame を生成する
fn arb_audio_frame() -> impl Strategy<Value = AudioFrame> {
    (
        any::<u32>(),
        arb_audio_format(),
        arb_audio_sample_rate(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(any::<u8>(), 0..256),
    )
        .prop_map(
            |(
                timestamp,
                format,
                sample_rate,
                is_8bit_sample,
                is_stereo,
                is_aac_sequence_header,
                data,
            )| {
                // AAC 以外の場合は is_aac_sequence_header は無視される (デコード時は false になる)
                let is_aac_sequence_header = if format == AudioFormat::Aac {
                    is_aac_sequence_header
                } else {
                    false
                };
                AudioFrame {
                    timestamp: RtmpTimestamp::from_millis(timestamp),
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

/// VideoFrameType を生成する
fn arb_video_frame_type() -> impl Strategy<Value = VideoFrameType> {
    prop_oneof![
        Just(VideoFrameType::KeyFrame),
        Just(VideoFrameType::InterFrame),
        Just(VideoFrameType::DisposableInterFrame),
        Just(VideoFrameType::GeneratedKeyFrame),
        Just(VideoFrameType::VideoInfoOrCommandFrame),
    ]
}

/// VideoCodec を生成する
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

/// AvcPacketType を生成する
fn arb_avc_packet_type() -> impl Strategy<Value = AvcPacketType> {
    prop_oneof![
        Just(AvcPacketType::SequenceHeader),
        Just(AvcPacketType::NalUnit),
        Just(AvcPacketType::EndOfSequence),
    ]
}

/// VideoFrame を生成する
fn arb_video_frame() -> impl Strategy<Value = VideoFrame> {
    (
        any::<u32>(),
        // composition_timestamp_offset は i24 (符号付き24ビット) なので範囲を制限
        -8388608i32..=8388607i32,
        arb_video_frame_type(),
        arb_video_codec(),
        prop::option::of(arb_avc_packet_type()),
        prop::collection::vec(any::<u8>(), 0..256),
    )
        .prop_map(
            |(timestamp, cts_offset, frame_type, codec, avc_packet_type, data)| {
                // AVC コーデックでかつ VideoInfoOrCommandFrame でない場合のみ avc_packet_type と cts_offset を使用
                let (avc_packet_type, composition_timestamp_offset) = if codec == VideoCodec::Avc
                    && frame_type != VideoFrameType::VideoInfoOrCommandFrame
                {
                    (
                        avc_packet_type.or(Some(AvcPacketType::NalUnit)),
                        RtmpTimestampDelta::from_millis(cts_offset),
                    )
                } else {
                    (None, RtmpTimestampDelta::ZERO)
                };
                VideoFrame {
                    timestamp: RtmpTimestamp::from_millis(timestamp),
                    composition_timestamp_offset,
                    frame_type,
                    codec,
                    avc_packet_type,
                    data,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// AudioFrame の encode / decode が可逆であることを検証
    #[test]
    fn audio_frame_roundtrip(frame in arb_audio_frame()) {
        let mut buf = Vec::new();
        encode_audio_frame(&mut buf, &frame);
        let decoded = decode_audio_frame(&buf, frame.timestamp).expect("decode should succeed");
        prop_assert_eq!(decoded, frame);
    }

    /// VideoFrame の encode / decode が可逆であることを検証
    #[test]
    fn video_frame_roundtrip(frame in arb_video_frame()) {
        let mut buf = Vec::new();
        encode_video_frame(&mut buf, &frame);
        let decoded = decode_video_frame(&buf, frame.timestamp).expect("decode should succeed");
        prop_assert_eq!(decoded, frame);
    }
}
