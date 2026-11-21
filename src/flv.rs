//! FLV の音声タグ・映像タグのヘッダーをエンコード・デコードするモジュール
//! データ形式は FLV 仕様（Video File Format Specification）に従う。

use alloc::vec::Vec;

use crate::bytes::{BytesReader, BytesWriter};
use crate::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
use crate::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, Error, VideoCodec, VideoFrame,
    VideoFrameType,
};

/// 音声フレームを FLV の音声タグの形式で `buf` の末尾に書き込む
///
/// 音声タグのヘッダー（AudioTagHeader）に音声フォーマット・サンプリングレート・
/// サンプルサイズ・チャンネル数を書き込み、その後に音声データを書き込む。
/// AAC の場合は AAC パケットタイプ（シーケンスヘッダーか生データか）も書き込む
/// （FLV 仕様 E.4.2 節）。
pub fn encode_audio_frame(buf: &mut Vec<u8>, frame: &AudioFrame) {
    // オーディオヘッダーバイトを構築
    let header = ((frame.format as u8) << 4)
        | ((frame.sample_rate as u8) << 2)
        | ((frame.is_8bit_sample as u8) << 1)
        | (frame.is_stereo as u8);

    // ヘッダーバイトを書き込む
    buf.write_u8(header);

    // フォーマットが AAC の場合、AAC 固有のヘッダーを書き込む
    if frame.format == AudioFormat::Aac {
        buf.write_u8(if frame.is_aac_sequence_header { 0 } else { 1 });
    }

    // オーディオデータを書き込む
    buf.write_bytes(&frame.data);
}

/// FLV の音声タグをデコードして [`AudioFrame`] を返す
///
/// 音声タグのヘッダー（AudioTagHeader）から音声フォーマット・サンプリングレート・
/// サンプルサイズ・チャンネル数を読み取り、残りのバイト列を音声データとして扱う。
/// `timestamp` はフレームのタイムスタンプとしてそのまま設定される。
/// AAC 以外のフォーマットでは `is_aac_sequence_header` は `false` になる。
/// 未知の音声フォーマットの場合や、AAC のパケットタイプを読み取れない場合は
/// [`Error`] を返す（FLV 仕様 E.4.2 節）。
pub fn decode_audio_frame(buf: &[u8], timestamp: RtmpTimestamp) -> Result<AudioFrame, Error> {
    let mut reader = buf;

    // オーディオヘッダーバイトを読み込む
    let header = reader.read_u8()?;

    // オーディオフォーマットを抽出 (ビット 7-4)
    let format_bits = (header >> 4) & 0x0F;
    let format = match format_bits {
        1 => AudioFormat::Adpcm,
        2 => AudioFormat::Mp3,
        3 => AudioFormat::LinearPcmLittleEndian,
        4 => AudioFormat::Nellymoser16khzMono,
        5 => AudioFormat::Nellymoser8KhzMono,
        6 => AudioFormat::Nellymoser,
        7 => AudioFormat::G711AlawLogarithmicPcm,
        8 => AudioFormat::G711MuLawLogarithmicPcm,
        10 => AudioFormat::Aac,
        11 => AudioFormat::Speex,
        14 => AudioFormat::Mp3_8khz,
        15 => AudioFormat::DeviceSpecificSound,
        _ => {
            return Err(Error::invalid_data(format!(
                "Invalid audio format: {}",
                format_bits
            )));
        }
    };

    // サンプリングレートを抽出 (ビット 3-2)
    let sample_rate_bits = (header >> 2) & 0x03;
    let sample_rate = match sample_rate_bits {
        0 => AudioSampleRate::Khz5,
        1 => AudioSampleRate::Khz11,
        2 => AudioSampleRate::Khz22,
        3 => AudioSampleRate::Khz44,
        _ => unreachable!(),
    };

    // サンプルサイズを抽出 (ビット 1)
    let is_8bit_sample = (header & 0x02) != 0;

    // チャンネルタイプを抽出 (ビット 0)
    let is_stereo = (header & 0x01) != 0;

    // フォーマットが AAC の場合、AAC 固有のヘッダーを処理する
    let is_aac_sequence_header = if format == AudioFormat::Aac {
        let aac_packet_type = reader.read_u8()?;
        aac_packet_type == 0 // 0 = シーケンスヘッダー、1 = 生データ
    } else {
        false
    };

    // 残りのオーディオデータを読み込む
    let data = reader.to_vec();

    Ok(AudioFrame {
        timestamp,
        format,
        sample_rate,
        is_8bit_sample,
        is_stereo,
        is_aac_sequence_header,
        data,
    })
}

/// 映像フレームを FLV の映像タグの形式で `buf` の末尾に書き込む
///
/// 映像タグのヘッダー（VideoTagHeader）にフレームタイプとコーデックを書き込み、
/// その後に映像データを書き込む。`avc_packet_type` が `Some` の場合は、
/// AVC パケットタイプと合成タイムスタンプオフセット（CompositionTime）も書き込む
/// （FLV 仕様 E.4.3 節）。
pub fn encode_video_frame(buf: &mut Vec<u8>, frame: &VideoFrame) {
    // フレームタイプとコーデックを組み合わせたバイトを構築
    let frame_type_codec = ((frame.frame_type as u8) << 4) | (frame.codec as u8);
    buf.write_u8(frame_type_codec);

    // AVC パケットタイプが設定されている場合、AVC 固有の情報を書き込む
    if let Some(avc_packet_type) = frame.avc_packet_type {
        buf.write_u8(avc_packet_type as u8);
        buf.write_i24(frame.composition_timestamp_offset.as_millis());
    }

    // ビデオデータを書き込む
    buf.write_bytes(&frame.data);
}

/// FLV の映像タグをデコードして [`VideoFrame`] を返す
///
/// 映像タグのヘッダー（VideoTagHeader）からフレームタイプとコーデックを読み取り、
/// 残りのバイト列を映像データとして扱う。`timestamp` はフレームのタイムスタンプとして
/// そのまま設定される。コーデックが AVC で、フレームタイプが
/// `VideoInfoOrCommandFrame` でない場合は、AVC パケットタイプと合成タイムスタンプ
/// オフセット（CompositionTime）も読み取る。未知のフレームタイプ・コーデック・
/// AVC パケットタイプの場合は [`Error`] を返す（FLV 仕様 E.4.3 節）。
pub fn decode_video_frame(buf: &[u8], timestamp: RtmpTimestamp) -> Result<VideoFrame, Error> {
    let mut reader = buf;

    // フレームタイプとコーデックを読み込む
    let frame_type_codec = reader.read_u8()?;

    // フレームタイプを抽出 (ビット 7-4)
    let frame_type_bits = (frame_type_codec >> 4) & 0x0F;
    let frame_type = match frame_type_bits {
        1 => VideoFrameType::KeyFrame,
        2 => VideoFrameType::InterFrame,
        3 => VideoFrameType::DisposableInterFrame,
        4 => VideoFrameType::GeneratedKeyFrame,
        5 => VideoFrameType::VideoInfoOrCommandFrame,
        _ => {
            return Err(Error::invalid_data(format!(
                "Invalid video frame type: {}",
                frame_type_bits
            )));
        }
    };

    // コーデック ID を抽出 (ビット 3-0)
    let codec_bits = frame_type_codec & 0x0F;
    let codec = match codec_bits {
        1 => VideoCodec::Jpeg,
        2 => VideoCodec::H263,
        3 => VideoCodec::ScreenVideo,
        4 => VideoCodec::Vp6,
        5 => VideoCodec::Vp6WithAlpha,
        6 => VideoCodec::ScreenVideoV2,
        7 => VideoCodec::Avc,
        _ => {
            return Err(Error::invalid_data(format!(
                "Invalid video codec: {}",
                codec_bits
            )));
        }
    };

    // コーデックが AVC で、フレームタイプが VideoInfoOrCommandFrame でない場合
    let (avc_packet_type, composition_timestamp_offset) =
        if codec == VideoCodec::Avc && frame_type != VideoFrameType::VideoInfoOrCommandFrame {
            let avc_packet_type_byte = reader.read_u8()?;
            let avc_packet_type = match avc_packet_type_byte {
                0 => AvcPacketType::SequenceHeader,
                1 => AvcPacketType::NalUnit,
                2 => AvcPacketType::EndOfSequence,
                _ => {
                    return Err(Error::invalid_data(format!(
                        "Invalid AVC packet type: {}",
                        avc_packet_type_byte
                    )));
                }
            };

            let composition_timestamp_offset = RtmpTimestampDelta::from_millis(reader.read_i24()?);
            (Some(avc_packet_type), composition_timestamp_offset)
        } else {
            // AVC 以外のコーデックや VideoInfoOrCommandFrame の場合
            (None, RtmpTimestampDelta::ZERO)
        };

    // 残りのビデオデータを読み込む
    let data = reader.to_vec();

    Ok(VideoFrame {
        timestamp,
        composition_timestamp_offset,
        frame_type,
        codec,
        avc_packet_type,
        data,
    })
}
