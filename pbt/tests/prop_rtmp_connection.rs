//! RTMP Connection (Message Channel) の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::error::ErrorKind;
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
};
use shiguredo_rtmp::rtmp_chunk::{MessageHeaderFormat, RtmpChunkSize};
use shiguredo_rtmp::rtmp_connection::RtmpMessageChannel;
use shiguredo_rtmp::rtmp_message::{RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId};
use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
use shiguredo_rtmp::rtmp_user_control_event::RtmpUserControlEvent;

#[path = "../../tests/helpers/rtmp_ignored_message.rs"]
pub mod rtmp_ignored_message;

/// このファイルの PBT ケース数
const CASES: usize = 300;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// Protocol Control Message 用のヘッダを生成する
fn sample_pcm_header(ctx: &mut TestCaseContext) -> RtmpMessageHeader {
    RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::PCM,
        timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
    }
}

/// Media Message 用のヘッダを生成する
fn sample_media_header(ctx: &mut TestCaseContext) -> RtmpMessageHeader {
    RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
    }
}

/// 有効なチャンクサイズを生成する
fn sample_chunk_size(ctx: &mut TestCaseContext) -> RtmpChunkSize {
    let size = noprop::sample_usize_in(ctx, 1..=4096);
    RtmpChunkSize::new(size).expect("サンプリングした範囲のチャンクサイズは有効")
}

/// ユーザーコントロールイベントを生成する
fn sample_user_control_event(ctx: &mut TestCaseContext) -> RtmpUserControlEvent {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1]) {
        0 => RtmpUserControlEvent::StreamBegin {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        1 => RtmpUserControlEvent::StreamEof {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        2 => RtmpUserControlEvent::StreamDry {
            stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        },
        3 => RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        _ => RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
    }
}

/// 音声フレームを生成する
fn sample_audio_frame(ctx: &mut TestCaseContext) -> AudioFrame {
    let format = noprop::sample_choice(ctx, &[AudioFormat::Mp3, AudioFormat::Aac]);
    let len = noprop::sample_usize_in(ctx, 0..64);
    // AAC 以外の場合、is_aac_sequence_header はデコード時に false になるため揃えておく
    let is_aac_sequence_header = format == AudioFormat::Aac && noprop::sample_bool(ctx);
    AudioFrame {
        timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        format,
        sample_rate: noprop::sample_choice(
            ctx,
            &[
                AudioSampleRate::Khz5,
                AudioSampleRate::Khz11,
                AudioSampleRate::Khz22,
                AudioSampleRate::Khz44,
            ],
        ),
        is_8bit_sample: noprop::sample_bool(ctx),
        is_stereo: noprop::sample_bool(ctx),
        is_aac_sequence_header,
        data: noprop::sample_bytes_vec(ctx, len),
    }
}

/// 映像フレームを生成する
fn sample_video_frame(ctx: &mut TestCaseContext) -> VideoFrame {
    let len = noprop::sample_usize_in(ctx, 0..64);
    if noprop::sample_bool(ctx) {
        // AVC 以外の映像フレーム
        VideoFrame {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
            composition_timestamp_offset: RtmpTimestampDelta::ZERO,
            frame_type: VideoFrameType::KeyFrame,
            codec: VideoCodec::Jpeg,
            avc_packet_type: None,
            data: noprop::sample_bytes_vec(ctx, len),
        }
    } else {
        // AVC の映像フレーム
        VideoFrame {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
            composition_timestamp_offset: RtmpTimestampDelta::from_millis(noprop::sample_i32_in(
                ctx,
                -1000..=1000,
            )),
            frame_type: VideoFrameType::KeyFrame,
            codec: VideoCodec::Avc,
            avc_packet_type: Some(noprop::sample_choice(
                ctx,
                &[
                    AvcPacketType::SequenceHeader,
                    AvcPacketType::NalUnit,
                    AvcPacketType::EndOfSequence,
                ],
            )),
            data: noprop::sample_bytes_vec(ctx, len),
        }
    }
}

/// 代表的な RTMP メッセージを生成する
fn sample_message(ctx: &mut TestCaseContext) -> RtmpMessage {
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1]) {
        0 => RtmpMessage::SetChunkSize {
            header: sample_pcm_header(ctx),
            size: sample_chunk_size(ctx),
        },
        1 => RtmpMessage::Ack {
            header: sample_pcm_header(ctx),
            sequence_number: noprop::sample_u32(ctx),
        },
        2 => RtmpMessage::WinAckSize {
            header: sample_pcm_header(ctx),
            size: noprop::sample_u32(ctx),
        },
        3 => RtmpMessage::UserControl {
            header: sample_pcm_header(ctx),
            event: sample_user_control_event(ctx),
        },
        4 => {
            let header = sample_media_header(ctx);
            let mut frame = sample_audio_frame(ctx);
            frame.timestamp = header.timestamp;
            RtmpMessage::Audio { header, frame }
        }
        _ => {
            let header = sample_media_header(ctx);
            let mut frame = sample_video_frame(ctx);
            frame.timestamp = header.timestamp;
            if frame.avc_packet_type.is_none() {
                frame.composition_timestamp_offset = RtmpTimestampDelta::ZERO;
            }
            RtmpMessage::Video { header, frame }
        }
    }
}

/// 指定した長さのメッセージ列を生成する
fn sample_messages(ctx: &mut TestCaseContext, range: std::ops::Range<usize>) -> Vec<RtmpMessage> {
    let len = noprop::sample_usize_in(ctx, range);
    (0..len).map(|_| sample_message(ctx)).collect()
}

/// RtmpMessageChannel の送信バッファが decode で再現できることを検証する
#[test]
fn message_channel_send_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let messages = sample_messages(ctx, 1..20);

        let mut channel = RtmpMessageChannel::default();
        for message in &messages {
            channel
                .feed_send_message(message.clone())
                .expect("生成した RTMP メッセージを送信バッファに追加できるはず");
        }

        // 一部ずつ送信済みにしても、未送信部分を欠落・重複させずに取り出せるはず。
        let buf = channel.send_buf().to_vec();
        let split = noprop::sample_usize_in(ctx, 1..buf.len());
        let mut decoder = RtmpMessageDecoder::default();
        decoder.feed_buf(&channel.send_buf()[..split]);
        channel.advance_send_buf(split);
        assert_eq!(
            channel.send_buf(),
            &buf[split..],
            "未送信部分だけが残るはず"
        );
        decoder.feed_buf(channel.send_buf());
        channel.advance_send_buf(buf.len() - split);
        assert!(channel.send_buf().is_empty(), "すべて送信済みのはず");

        let mut decoded = Vec::new();
        while let Some(message) = decoder
            .decode()
            .expect("エンコードしたメッセージのデコードは失敗しない")
        {
            decoded.push(message);
        }

        assert_eq!(
            decoded, messages,
            "ラウンドトリップしたメッセージ列が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// 不正な映像メッセージは送信待ちのバイト列と後続のチャンクヘッダー圧縮に影響しない
#[test]
fn message_channel_rejects_invalid_video_without_changing_output() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let stream_id = RtmpMessageStreamId::new(noprop::sample_u32_in(ctx, 1..=500));
        let step_ms = noprop::sample_u32_in(ctx, 1..=10_000);
        let chunk_size = RtmpChunkSize::new(noprop::sample_usize_in(ctx, 3..=16))
            .expect("既定値より小さい有効なチャンクサイズを生成する");
        let mut frame = sample_video_frame(ctx);
        // 既定値では分割されず、設定したサイズでは必ず分割される映像メッセージを使う。
        frame.data.resize(frame.data.len().max(chunk_size.get()), 0);
        let video_at = |timestamp_ms, frame| {
            let timestamp = RtmpTimestamp::from_millis(timestamp_ms);
            RtmpMessage::Video {
                header: RtmpMessageHeader {
                    stream_id,
                    timestamp,
                },
                frame: VideoFrame { timestamp, ..frame },
            }
        };
        let first = video_at(3 * step_ms, frame.clone());
        let next = video_at(6 * step_ms, frame.clone());

        let setup = || {
            let mut channel = RtmpMessageChannel::default();
            channel
                .feed_send_message(RtmpMessage::set_chunk_size(chunk_size))
                .expect("チャンクサイズの変更を追加できるはず");
            channel
                .feed_send_message(first.clone())
                .expect("先行する有効な映像メッセージを追加できるはず");
            channel
        };
        let mut control = setup();
        let pending_len = control.send_buf().len();
        control
            .feed_send_message(next.clone())
            .expect("後続の有効な映像メッセージを追加できるはず");
        // RTMP 仕様 5.3.1.2 節の形式 3 を使うように、型・長さと時刻差分をそろえる。
        // 圧縮ヘッダーの条件が変わった場合は、履歴が反映される比較方法を見直す。
        assert_eq!(
            control.send_buf()[pending_len] >> 6,
            MessageHeaderFormat::F3 as u8,
            "後続メッセージは形式 3 の圧縮ヘッダーを使うはず"
        );

        // パケットタイプの不整合と SI24 の範囲外を、同じ先行履歴から個別に検証する。
        let nalu = Some(AvcPacketType::NalUnit);
        for (codec, avc_packet_type, offset) in [
            (VideoCodec::Avc, None, 0),
            (VideoCodec::Jpeg, nalu, 0),
            (VideoCodec::Avc, nalu, -8_388_609),
            (VideoCodec::Avc, nalu, 8_388_608),
            (VideoCodec::Avc, nalu, 0x0100_0000),
        ] {
            let mut channel = setup();
            let pending = channel.send_buf().to_vec();
            // 不正な時刻が履歴に残ると、次の有効メッセージの時刻差分が変わる。
            let invalid = video_at(
                4 * step_ms,
                VideoFrame {
                    codec,
                    avc_packet_type,
                    composition_timestamp_offset: RtmpTimestampDelta::from_millis(offset),
                    ..frame.clone()
                },
            );
            let error = channel
                .feed_send_message(invalid)
                .expect_err("ヘッダーを正しく書けない映像は拒否するはず");
            assert_eq!(
                error.kind,
                ErrorKind::InvalidInput,
                "不正入力のエラー種別: {codec:?}, {avc_packet_type:?}, {offset}"
            );
            assert_eq!(
                channel.send_buf(),
                pending,
                "失敗しても送信待ちのバイト列は変わらないはず"
            );

            channel
                .feed_send_message(next.clone())
                .expect("失敗後も有効な映像メッセージを追加できるはず");
            assert_eq!(
                channel.send_buf(),
                control.send_buf(),
                "失敗しなかった対照とチャンク分割・圧縮ヘッダーを含む送信バイト列が一致するはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// チャネルは無視対象を飛ばして次の通常メッセージを返し、ACK には受信した全バイトを数える
#[test]
fn message_channel_skips_ignored_messages_and_counts_their_bytes() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        for sample in rtmp_ignored_message::sample_ignored_messages(ctx) {
            let mut channel = RtmpMessageChannel::default();
            // メッセージの完成とは無関係に、各入力だけで ACK 閾値を超えるようにする。
            let incomplete_len = sample.incomplete.len() as u32;
            let completion_len = sample.completion.len() as u32;
            channel.set_peer_ack_window_size(2 * incomplete_len.min(completion_len) - 1);
            assert_eq!(
                channel
                    .feed_recv_buf(&sample.incomplete)
                    .expect("未完成の入力も受理するはず"),
                Some(incomplete_len),
                "組み立てが完了する前に受信済みバイト数を ACK に含めるはず"
            );
            assert_eq!(
                channel.next_recv_message(),
                None,
                "未完成のメッセージは取り出せないはず"
            );

            let total_bytes = incomplete_len + completion_len;
            assert_eq!(
                channel
                    .feed_recv_buf(&sample.completion)
                    .expect("残りの入力も受理するはず"),
                Some(total_bytes),
                "ACK には無視したメッセージのチャンクも含めるはず"
            );
            // キュー途中の無視対象が原因で None を返すと、後続の音声を取り出せない。
            for expected in sample.into_audio_messages() {
                assert_eq!(
                    channel.next_recv_message(),
                    Some(expected),
                    "対応済みメッセージを追加の入力なしで完成順に取り出せるはず"
                );
            }
            assert_eq!(
                channel.next_recv_message(),
                None,
                "無視対象は通常メッセージとして返さないはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// 受信バイト数と peer_ack_window_size に基づく ACK 送信判定を検証する
#[test]
fn message_channel_recv_ack_threshold() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let messages = sample_messages(ctx, 1..10);
        let segment_count = noprop::sample_usize_in(ctx, 1..40);
        let peer_ack_window_size = noprop::sample_u32_in(ctx, 1..=10_000);

        let mut sender = RtmpMessageChannel::default();
        for message in &messages {
            sender
                .feed_send_message(message.clone())
                .expect("生成した RTMP メッセージを送信バッファに追加できるはず");
        }
        let buf = sender.send_buf().to_vec();

        let mut receiver = RtmpMessageChannel::default();
        receiver.set_peer_ack_window_size(peer_ack_window_size);

        let mut total_received: u32 = 0;
        let mut last_ack_sent: u32 = 0;
        let mut offset = 0usize;

        for _ in 0..segment_count {
            if offset >= buf.len() {
                break;
            }

            let size = noprop::sample_usize_in(ctx, 0..=128);
            let remaining = buf.len() - offset;
            let take = size.min(remaining);
            let chunk = &buf[offset..offset + take];
            offset += take;

            let ack = receiver
                .feed_recv_buf(chunk)
                .expect("送信バッファを分割したデータのデコードは失敗しない");

            total_received += take as u32;
            let unacked = total_received - last_ack_sent;
            if unacked > peer_ack_window_size / 2 {
                assert_eq!(
                    ack,
                    Some(total_received),
                    "ACK を送信すべきタイミングで送信されていない"
                );
                last_ack_sent = total_received;
            } else {
                assert_eq!(ack, None, "ACK を送信すべきでないタイミングで送信された");
            }
        }
        // 最終 ACK の値が受信バイト数を超えないことを確認する
        assert!(
            last_ack_sent <= total_received,
            "最終 ACK の値が受信バイト数を超えている"
        );
        Ok(())
    })?;
    Ok(())
}
