//! FLV (音声・映像フレーム) の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::error::ErrorKind;
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

/// AVC 以外の VideoCodec を生成する
fn sample_non_avc_video_codec(ctx: &mut TestCaseContext) -> VideoCodec {
    noprop::sample_choice(
        ctx,
        &[
            VideoCodec::Jpeg,
            VideoCodec::H263,
            VideoCodec::ScreenVideo,
            VideoCodec::Vp6,
            VideoCodec::Vp6WithAlpha,
            VideoCodec::ScreenVideoV2,
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
    let frame_type = sample_video_frame_type(ctx);
    // AVC 追加ヘッダーの有無を同じ割合で生成する。
    let codec = if noprop::sample_bool(ctx) {
        VideoCodec::Avc
    } else {
        sample_non_avc_video_codec(ctx)
    };
    let len = noprop::sample_usize_in(ctx, 0..256);

    // コマンドでも AVC の場合は追加ヘッダーを持つ。非 AVC のオフセットはワイヤに現れない。
    let (avc_packet_type, composition_timestamp_offset) = if codec == VideoCodec::Avc {
        // i24 (符号付き 24 ビット) の範囲に収まる値を生成する。
        let cts_offset = noprop::sample_i32_in(ctx, -8_388_608..=8_388_607);
        (
            Some(sample_avc_packet_type(ctx)),
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

/// PCM の SoundSize の対応を、往復ではなく独立した固定ヘッダーで検証する
#[test]
fn pcm_sound_size_matches_wire_bits() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
        let prefix_len = noprop::sample_usize_in(ctx, 0..64);
        let prefix = noprop::sample_bytes_vec(ctx, prefix_len);
        // Adobe FLV 仕様 10.1 の E.4.2.1 節: リトルエンディアン PCM、44 kHz、モノラル。
        // ヘッダーの他のビットは固定し、8 ビットと 16 ビットの両方を毎回確認する。
        for (is_8bit_sample, expected) in
            [(true, &[0x3C, 0x80][..]), (false, &[0x3E, 0x34, 0x12][..])]
        {
            let frame = AudioFrame {
                timestamp,
                format: AudioFormat::LinearPcmLittleEndian,
                sample_rate: AudioSampleRate::Khz44,
                is_8bit_sample,
                is_stereo: false,
                is_aac_sequence_header: false,
                data: expected[1..].to_vec(),
            };
            let sample_size = if is_8bit_sample { 8 } else { 16 };
            let mut wire = prefix.clone();
            encode_audio_frame(&mut wire, &frame);
            assert_eq!(
                wire,
                [prefix.as_slice(), expected].concat(),
                "{sample_size} ビット PCM の固定ワイヤ値を既存バッファに追記するはず"
            );
            assert_eq!(
                decode_audio_frame(expected, timestamp)
                    .expect("仕様どおりの PCM タグをデコードできるはず"),
                frame,
                "{sample_size} ビット PCM の固定ワイヤ値からフレームを得るはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// Speex は受信したビットと送信フレームのフラグにかかわらず SoundSize 1 として扱う
#[test]
fn speex_sound_size_matches_wire_bits() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
        let len = noprop::sample_usize_in(ctx, 1..256);
        let data = noprop::sample_bytes_vec(ctx, len);
        // Adobe FLV 仕様 10.1 の E.4.2.1 節では、Speex のヘッダーはレート 0、サイズ 1、モノラル。
        let mut frame = AudioFrame {
            timestamp,
            format: AudioFormat::Speex,
            sample_rate: AudioSampleRate::Khz5,
            is_8bit_sample: false,
            is_stereo: false,
            is_aac_sequence_header: false,
            data,
        };
        let expected = [&[0xB2][..], frame.data.as_slice()].concat();
        for input_header in [0xB0, 0xB2] {
            let input = [&[input_header][..], frame.data.as_slice()].concat();
            assert_eq!(
                decode_audio_frame(&input, timestamp).expect("Speex タグを読み取れるはず"),
                frame,
                "Speex の入力ビットは使わず、ペイロードは保持するはず"
            );
        }
        for is_8bit_sample in [true, false] {
            frame.is_8bit_sample = is_8bit_sample;
            let mut output = Vec::new();
            encode_audio_frame(&mut output, &frame);
            assert_eq!(
                output, expected,
                "Speex は SoundSize を 1 としてペイロードをそのまま書くはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// AudioFrame の往復を、Speex の送信時の正規化を含めて検証する
#[test]
fn audio_frame_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let frame = sample_audio_frame(ctx);
        let mut buf = Vec::new();
        encode_audio_frame(&mut buf, &frame);
        // 往復だけでは極性の誤りを見逃す。Speex も含め、書き込んだビットを直接調べる。
        let expected_sound_size = match (frame.format, frame.is_8bit_sample) {
            (AudioFormat::Speex, _) | (_, false) => 0x02,
            (_, true) => 0x00,
        };
        assert_eq!(
            buf.first().map(|header| header & 0x02),
            Some(expected_sound_size),
            "指定した SoundSize を書くはず"
        );
        let decoded = decode_audio_frame(&buf, frame.timestamp)
            .expect("エンコードした音声フレームのデコードは失敗しない");
        let expected = AudioFrame {
            is_8bit_sample: expected_sound_size == 0,
            ..frame
        };
        assert_eq!(decoded, expected, "音声フレームが期待した値で往復するはず");
        Ok(())
    })?;
    Ok(())
}

/// VideoFrame の encode / decode が可逆であることを検証する
#[test]
fn video_frame_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let mut frame = sample_video_frame(ctx);
        let _ = assert_video_frame_roundtrip(&frame);

        // 境界値は乱数に任せず、生成したタイムスタンプとデータに対して毎回検証する。
        frame.frame_type = VideoFrameType::InterFrame;
        frame.codec = VideoCodec::Avc;
        frame.avc_packet_type = Some(AvcPacketType::NalUnit);
        for (offset, expected) in [
            (-8_388_608, [0x80, 0x00, 0x00]),
            (8_388_607, [0x7f, 0xff, 0xff]),
        ] {
            frame.composition_timestamp_offset = RtmpTimestampDelta::from_millis(offset);
            let buf = assert_video_frame_roundtrip(&frame);
            assert_eq!(
                buf.get(2..5),
                Some(expected.as_slice()),
                "CompositionTime の境界値 {offset} のワイヤ表現が一致しない"
            );
        }
        Ok(())
    })?;
    Ok(())
}

fn assert_video_frame_roundtrip(frame: &VideoFrame) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_video_frame(&mut buf, frame).expect("有効な映像フレームのエンコードは失敗しない");

    // 往復だけでは両方向の同じ誤りを検出できないので、AVC 追加ヘッダーを別に照合する。
    let body_start = if let Some(packet_type) = frame.avc_packet_type {
        let type_byte = match packet_type {
            AvcPacketType::SequenceHeader => 0,
            AvcPacketType::NalUnit => 1,
            AvcPacketType::EndOfSequence => 2,
        };
        let [_, high, mid, low] = frame.composition_timestamp_offset.as_millis().to_be_bytes();
        assert_eq!(
            buf.get(1..5),
            Some([type_byte, high, mid, low].as_slice()),
            "AVC 追加ヘッダーのワイヤ表現が一致しない"
        );
        5
    } else {
        1
    };
    assert_eq!(
        buf.get(body_start..),
        Some(frame.data.as_slice()),
        "コーデックに応じた長さのヘッダーにボディが続くはず"
    );

    let decoded = decode_video_frame(&buf, frame.timestamp)
        .expect("エンコードした映像フレームのデコードは失敗しない");
    assert_eq!(
        &decoded, frame,
        "ラウンドトリップした映像フレームが一致しない"
    );
    buf
}

/// コマンドのワイヤ表現は既存バッファやタイムスタンプによらず仕様に従う
#[test]
fn video_command_wire_format() -> noprop::TestResult {
    const AVC_COMMAND: &[u8] = &[0x57, 0x01, 0x00, 0x00, 0x00, 0x01];
    // パケットタイプまたは SI24 が途中で終わるすべての位置で拒否する。
    for len in 1..5 {
        assert!(
            decode_video_frame(&AVC_COMMAND[..len], RtmpTimestamp::ZERO).is_err(),
            "AVC 追加ヘッダーが途中で終わる {len} バイトの入力は拒否するはず"
        );
    }

    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
        let prefix_len = noprop::sample_usize_in(ctx, 0..256);
        let prefix = noprop::sample_bytes_vec(ctx, prefix_len);

        // Adobe FLV 仕様 10.1 §E.4.3.1（将来の改訂で変更される可能性がある）。
        // FrameType が 5 でも、CodecID が 7 ならコマンドの前に 4 バイトの追加ヘッダーを置く。
        for (codec, avc_packet_type, command, wire) in [
            (
                VideoCodec::Avc,
                Some(AvcPacketType::NalUnit),
                0x01,
                AVC_COMMAND,
            ),
            // 異なる値も使い、パケットタイプとコマンドの位置が入れ替わる退行を検出する。
            (
                VideoCodec::Avc,
                Some(AvcPacketType::NalUnit),
                0x00,
                &[0x57, 0x01, 0x00, 0x00, 0x00, 0x00][..],
            ),
            (VideoCodec::Vp6, None, 0x01, &[0x54, 0x01][..]),
        ] {
            let frame = VideoFrame {
                timestamp,
                composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                frame_type: VideoFrameType::VideoInfoOrCommandFrame,
                codec,
                avc_packet_type,
                data: vec![command],
            };

            let mut buf = prefix.clone();
            encode_video_frame(&mut buf, &frame)
                .expect("有効な映像コマンドのエンコードは失敗しない");
            assert_eq!(
                buf,
                [prefix.as_slice(), wire].concat(),
                "既存バッファまたは映像コマンドのワイヤ表現が一致しない: {codec:?}"
            );

            // 両方向が同じ誤りを持つ場合も検出できるように、デコードは固定のワイヤ表現を使う。
            let decoded = decode_video_frame(wire, timestamp)
                .expect("有効な映像コマンドのデコードは失敗しない");
            assert_eq!(
                decoded, frame,
                "デコードした映像コマンドが一致しない: {codec:?}"
            );
        }

        Ok(())
    })?;
    Ok(())
}

/// SI24 の範囲外の差分は、追加ヘッダーを書き出す AVC の場合だけ拒否する
#[test]
fn composition_time_range_applies_only_to_avc() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));
        let prefix_len = noprop::sample_usize_in(ctx, 1..=32);
        let prefix = noprop::sample_bytes_vec(ctx, prefix_len);
        let payload = noprop::sample_bytes_vec(ctx, 8);
        let non_avc_codec = sample_non_avc_video_codec(ctx);

        for offset in [
            -8_388_609,
            8_388_608,
            0x0100_0000,
            i32::MIN,
            i32::MAX,
            noprop::sample_i32_in(ctx, i32::MIN..=-8_388_609),
            noprop::sample_i32_in(ctx, 8_388_608..=i32::MAX),
        ] {
            let delta = RtmpTimestampDelta::from_millis(offset);
            // ヘッダーを書く全パケット種別に範囲検査が適用され、コマンドでも省略しない。
            for (frame_type, packet_type, data) in [
                (
                    VideoFrameType::KeyFrame,
                    AvcPacketType::SequenceHeader,
                    payload.as_slice(),
                ),
                (VideoFrameType::InterFrame, AvcPacketType::NalUnit, &[]),
                (
                    VideoFrameType::InterFrame,
                    AvcPacketType::EndOfSequence,
                    &[],
                ),
                (
                    VideoFrameType::VideoInfoOrCommandFrame,
                    AvcPacketType::NalUnit,
                    &[0x01],
                ),
            ] {
                let frame = VideoFrame {
                    timestamp,
                    composition_timestamp_offset: delta,
                    frame_type,
                    codec: VideoCodec::Avc,
                    avc_packet_type: Some(packet_type),
                    data: data.to_vec(),
                };
                let mut buf = prefix.clone();
                let error = encode_video_frame(&mut buf, &frame)
                    .expect_err("SI24 で表せない AVC のオフセットを拒否するはず");
                assert_eq!(
                    error.kind,
                    ErrorKind::InvalidInput,
                    "範囲外のオフセットのエラー種別: {frame_type:?}, {packet_type:?}, {offset}"
                );
                assert_eq!(buf, prefix, "不正なオフセットで既存バッファを変えない");
            }

            // 非 AVC では同じ値を渡しても、ヘッダーに含めないのでエラーにならない。
            let frame = VideoFrame {
                timestamp,
                composition_timestamp_offset: delta,
                frame_type: VideoFrameType::InterFrame,
                codec: non_avc_codec,
                avc_packet_type: None,
                data: payload.clone(),
            };
            let mut buf = Vec::new();
            encode_video_frame(&mut buf, &frame).expect("非 AVC のオフセットは検査しないはず");
            assert_eq!(
                buf.get(1..),
                Some(payload.as_slice()),
                "追加ヘッダーを書かない"
            );
            let decoded =
                decode_video_frame(&buf, timestamp).expect("非 AVC のタグは復号できるはず");
            assert_eq!(
                decoded.composition_timestamp_offset,
                RtmpTimestampDelta::ZERO
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// AVC パケットタイプとコーデックが矛盾すると、既存バッファを書き換えずに失敗する
#[test]
fn video_frame_rejects_inconsistent_avc_packet_type() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let prefix_len = noprop::sample_usize_in(ctx, 1..256);
        let prefix = noprop::sample_bytes_vec(ctx, prefix_len);
        let frame_type = sample_video_frame_type(ctx);
        let timestamp = RtmpTimestamp::from_millis(noprop::sample_u32(ctx));

        // 非 AVC のパケットタイプ指定と AVC の指定漏れの両方を、各生成ケースで検証する。
        for (codec, avc_packet_type) in [
            (
                sample_non_avc_video_codec(ctx),
                Some(sample_avc_packet_type(ctx)),
            ),
            (VideoCodec::Avc, None),
        ] {
            let frame = VideoFrame {
                timestamp,
                composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                frame_type,
                codec,
                avc_packet_type,
                data: vec![0x01],
            };
            let mut buf = prefix.clone();
            let err = encode_video_frame(&mut buf, &frame).expect_err(
                "AVC パケットタイプとコーデックが矛盾する映像フレームはエンコードできない",
            );
            assert_eq!(
                err.kind,
                ErrorKind::InvalidInput,
                "AVC パケットタイプの不整合を示すエラー種別が異なる: {codec:?}"
            );
            assert_eq!(
                buf, prefix,
                "エンコード失敗時に既存バッファが変化した: {codec:?}"
            );
        }
        Ok(())
    })?;
    Ok(())
}
