//! FLV (音声・映像フレーム) の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::flv::{
    decode_audio_frame, decode_video_frame, encode_audio_frame, encode_video_frame,
};
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
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
    let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
    let format = sample_audio_format(ctx);
    let len = noprop::sample_usize_in(ctx, 0..256);
    // AAC 以外の場合、is_aac_sequence_header はデコード時に false になるため揃えておく
    let is_aac_sequence_header = format == AudioFormat::Aac && noprop::sample_bool(ctx);
    AudioFrame {
        timestamp,
        format,
        sample_rate: sample_audio_sample_rate(ctx),
        is_8bit_sample: noprop::sample_bool(ctx),
        is_stereo: noprop::sample_bool(ctx),
        is_aac_sequence_header,
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
    let len = noprop::sample_usize_in(ctx, 0..256);

    // AVC コーデックでかつ VideoInfoOrCommandFrame でない場合のみ
    // avc_packet_type と composition_timestamp_offset を使用する
    let (avc_packet_type, composition_timestamp_offset) =
        if codec == VideoCodec::Avc && frame_type != VideoFrameType::VideoInfoOrCommandFrame {
            (
                avc_packet_type.or(Some(AvcPacketType::NalUnit)),
                RtmpTimestampDelta::from_millis(cts_offset),
            )
        } else {
            (None, RtmpTimestampDelta::ZERO)
        };

    VideoFrame {
        timestamp,
        composition_timestamp_offset,
        frame_type,
        codec,
        avc_packet_type,
        data: noprop::sample_bytes_vec(ctx, len),
    }
}

/// AudioFrame の encode / decode が可逆であることを検証する
#[test]
fn audio_frame_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_audio_frame(ctx);
        let mut buf = Vec::new();
        encode_audio_frame(&mut buf, &frame);
        let decoded = decode_audio_frame(&buf, frame.timestamp)
            .expect("エンコードした音声フレームのデコードは失敗しない");
        assert_eq!(
            decoded, frame,
            "ラウンドトリップした音声フレームが一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// VideoFrame の encode / decode が可逆であることを検証する
#[test]
fn video_frame_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_video_frame(ctx);
        let mut buf = Vec::new();
        encode_video_frame(&mut buf, &frame);
        let decoded = decode_video_frame(&buf, frame.timestamp)
            .expect("エンコードした映像フレームのデコードは失敗しない");
        assert_eq!(
            decoded, frame,
            "ラウンドトリップした映像フレームが一致しない"
        );
        Ok(())
    })?;
    Ok(())
}
