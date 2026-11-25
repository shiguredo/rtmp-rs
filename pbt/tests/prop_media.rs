//! Media (AudioFrame, VideoFrame, MediaFrame) の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, MediaFrame, VideoCodec, VideoFrame,
    VideoFrameType,
};
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};

/// このファイルの PBT ケース数
const CASES: usize = 300;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// AudioFormat を生成する
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

/// AudioSampleRate を生成する
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

/// AudioFrame を生成する
fn sample_audio_frame(ctx: &mut TestCaseContext) -> AudioFrame {
    let len = noprop::sample_usize_in(ctx, 0..64);
    AudioFrame {
        timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        format: sample_audio_format(ctx),
        sample_rate: sample_audio_sample_rate(ctx),
        is_8bit_sample: noprop::sample_bool(ctx),
        is_stereo: noprop::sample_bool(ctx),
        is_aac_sequence_header: noprop::sample_bool(ctx),
        data: noprop::sample_bytes_vec(ctx, len),
    }
}

/// VideoFrameType を生成する
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

/// VideoCodec を生成する
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

/// AvcPacketType を生成する
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

/// VideoFrame を生成する
fn sample_video_frame(ctx: &mut TestCaseContext) -> VideoFrame {
    let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
    // i24 (符号付き 24 ビット) の範囲に収まる値を生成する
    let cts_offset = noprop::sample_i32_in(ctx, -8_388_608..=8_388_607);
    let frame_type = sample_video_frame_type(ctx);
    let codec = sample_video_codec(ctx);
    let avc_packet_type = if noprop::sample_bool(ctx) {
        Some(sample_avc_packet_type(ctx))
    } else {
        None
    };
    let len = noprop::sample_usize_in(ctx, 0..64);
    VideoFrame {
        timestamp,
        composition_timestamp_offset: RtmpTimestampDelta::from_millis(cts_offset),
        frame_type,
        codec,
        avc_packet_type,
        data: noprop::sample_bytes_vec(ctx, len),
    }
}

/// MediaFrame を生成する
fn sample_media_frame(ctx: &mut TestCaseContext) -> MediaFrame {
    match noprop::sample_weighted_index(ctx, &[1, 1]) {
        0 => MediaFrame::Audio(sample_audio_frame(ctx)),
        _ => MediaFrame::Video(sample_video_frame(ctx)),
    }
}

/// VideoFrame::is_keyframe() が frame_type と一致することを検証する
#[test]
fn video_frame_is_keyframe() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_video_frame(ctx);
        let expected = frame.frame_type == VideoFrameType::KeyFrame;
        assert_eq!(
            frame.is_keyframe(),
            expected,
            "キーフレーム判定が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// MediaFrame::Audio が AudioFrame を保持することを検証する
#[test]
fn media_frame_audio_variant() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let audio_frame = sample_audio_frame(ctx);
        let media = MediaFrame::Audio(audio_frame.clone());
        match media {
            MediaFrame::Audio(inner) => {
                assert_eq!(inner, audio_frame, "保持している音声フレームが一致しない")
            }
            MediaFrame::Video(_) => panic!("Audio variant が期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// MediaFrame::Video が VideoFrame を保持することを検証する
#[test]
fn media_frame_video_variant() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let video_frame = sample_video_frame(ctx);
        let media = MediaFrame::Video(video_frame.clone());
        match media {
            MediaFrame::Video(inner) => {
                assert_eq!(inner, video_frame, "保持している映像フレームが一致しない")
            }
            MediaFrame::Audio(_) => panic!("Video variant が期待される"),
        }
        Ok(())
    })?;
    Ok(())
}

/// MediaFrame の Clone が元の値と一致することを検証する
#[test]
fn media_frame_clone() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_media_frame(ctx);
        let cloned = frame.clone();
        assert_eq!(cloned, frame, "Clone した MediaFrame が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AudioFrame の Clone が元の値と一致することを検証する
#[test]
fn audio_frame_clone() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_audio_frame(ctx);
        let cloned = frame.clone();
        assert_eq!(cloned, frame, "Clone した AudioFrame が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// VideoFrame の Clone が元の値と一致することを検証する
#[test]
fn video_frame_clone() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_video_frame(ctx);
        let cloned = frame.clone();
        assert_eq!(cloned, frame, "Clone した VideoFrame が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AudioFormat の判別値がライブラリの扱う範囲にあることを検証する
#[test]
fn audio_format_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_audio_format(ctx) as u8;
        assert!(
            matches!(value, 0..=8 | 10 | 11 | 14 | 15),
            "音声フォーマットの判別値 {value} が対応範囲外"
        );
        Ok(())
    })?;
    Ok(())
}

/// AudioSampleRate の判別値が 0-3 の範囲にあることを検証する
#[test]
fn audio_sample_rate_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_audio_sample_rate(ctx) as u8;
        assert!(value <= 3, "サンプリングレートの値 {value} が有効範囲外");
        Ok(())
    })?;
    Ok(())
}

/// VideoCodec の判別値が 1-7 の範囲にあることを検証する
#[test]
fn video_codec_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_video_codec(ctx) as u8;
        assert!(
            (1..=7).contains(&value),
            "映像コーデックの値 {value} が有効範囲外"
        );
        Ok(())
    })?;
    Ok(())
}

/// VideoFrameType の判別値が 1-5 の範囲にあることを検証する
#[test]
fn video_frame_type_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_video_frame_type(ctx) as u8;
        assert!(
            (1..=5).contains(&value),
            "映像フレーム種別の値 {value} が有効範囲外"
        );
        Ok(())
    })?;
    Ok(())
}

/// AvcPacketType の判別値が 0-2 の範囲にあることを検証する
#[test]
fn avc_packet_type_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_avc_packet_type(ctx) as u8;
        assert!(value <= 2, "AVC パケット種別の値 {value} が有効範囲外");
        Ok(())
    })?;
    Ok(())
}
