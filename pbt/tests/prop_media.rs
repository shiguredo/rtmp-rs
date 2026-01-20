//! Media (AudioFrame, VideoFrame, MediaFrame) の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, MediaFrame, RtmpTimestamp,
    RtmpTimestampDelta, VideoCodec, VideoFrame, VideoFrameType,
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
        prop::collection::vec(any::<u8>(), 0..64),
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
        -8388608i32..=8388607i32,
        arb_video_frame_type(),
        arb_video_codec(),
        prop::option::of(arb_avc_packet_type()),
        prop::collection::vec(any::<u8>(), 0..64),
    )
        .prop_map(
            |(timestamp, cts_offset, frame_type, codec, avc_packet_type, data)| VideoFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp),
                composition_timestamp_offset: RtmpTimestampDelta::from_millis(cts_offset),
                frame_type,
                codec,
                avc_packet_type,
                data,
            },
        )
}

/// MediaFrame を生成する
fn arb_media_frame() -> impl Strategy<Value = MediaFrame> {
    prop_oneof![
        arb_audio_frame().prop_map(MediaFrame::Audio),
        arb_video_frame().prop_map(MediaFrame::Video),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// VideoFrame::is_keyframe() が正しく動作することを検証
    #[test]
    fn video_frame_is_keyframe(frame in arb_video_frame()) {
        let expected = frame.frame_type == VideoFrameType::KeyFrame;
        prop_assert_eq!(frame.is_keyframe(), expected);
    }

    /// MediaFrame::Audio が AudioFrame を正しく保持することを検証
    #[test]
    fn media_frame_audio_variant(audio_frame in arb_audio_frame()) {
        let media = MediaFrame::Audio(audio_frame.clone());
        match media {
            MediaFrame::Audio(inner) => prop_assert_eq!(inner, audio_frame),
            MediaFrame::Video(_) => prop_assert!(false, "Expected Audio variant"),
        }
    }

    /// MediaFrame::Video が VideoFrame を正しく保持することを検証
    #[test]
    fn media_frame_video_variant(video_frame in arb_video_frame()) {
        let media = MediaFrame::Video(video_frame.clone());
        match media {
            MediaFrame::Video(inner) => prop_assert_eq!(inner, video_frame),
            MediaFrame::Audio(_) => prop_assert!(false, "Expected Video variant"),
        }
    }

    /// MediaFrame の Clone が正しく動作することを検証
    #[test]
    fn media_frame_clone(frame in arb_media_frame()) {
        let cloned = frame.clone();
        prop_assert_eq!(cloned, frame);
    }

    /// AudioFrame の Clone が正しく動作することを検証
    #[test]
    fn audio_frame_clone(frame in arb_audio_frame()) {
        let cloned = frame.clone();
        prop_assert_eq!(cloned, frame);
    }

    /// VideoFrame の Clone が正しく動作することを検証
    #[test]
    fn video_frame_clone(frame in arb_video_frame()) {
        let cloned = frame.clone();
        prop_assert_eq!(cloned, frame);
    }

    /// AudioFormat の値が正しい範囲にあることを検証
    #[test]
    fn audio_format_values(format in arb_audio_format()) {
        let value = format as u8;
        // 有効な値: 1-8, 10, 11, 14, 15
        prop_assert!(
            matches!(value, 1..=8 | 10 | 11 | 14 | 15),
            "Invalid audio format value: {}", value
        );
    }

    /// AudioSampleRate の値が 0-3 の範囲にあることを検証
    #[test]
    fn audio_sample_rate_values(rate in arb_audio_sample_rate()) {
        let value = rate as u8;
        prop_assert!(value <= 3, "Invalid sample rate value: {}", value);
    }

    /// VideoCodec の値が 1-7 の範囲にあることを検証
    #[test]
    fn video_codec_values(codec in arb_video_codec()) {
        let value = codec as u8;
        prop_assert!((1..=7).contains(&value), "Invalid video codec value: {}", value);
    }

    /// VideoFrameType の値が 1-5 の範囲にあることを検証
    #[test]
    fn video_frame_type_values(frame_type in arb_video_frame_type()) {
        let value = frame_type as u8;
        prop_assert!((1..=5).contains(&value), "Invalid video frame type value: {}", value);
    }

    /// AvcPacketType の値が 0-2 の範囲にあることを検証
    #[test]
    fn avc_packet_type_values(packet_type in arb_avc_packet_type()) {
        let value = packet_type as u8;
        prop_assert!(value <= 2, "Invalid AVC packet type value: {}", value);
    }
}
