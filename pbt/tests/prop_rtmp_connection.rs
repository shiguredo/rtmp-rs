//! RTMP Connection (Message Channel) の Property-Based Testing

use proptest::collection::vec;
use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, RtmpChunkSize, RtmpMessage,
    RtmpMessageChannel, RtmpMessageDecoder, RtmpMessageHeader, RtmpMessageStreamId, RtmpTimestamp,
    RtmpTimestampDelta, RtmpUserControlEvent, VideoCodec, VideoFrame, VideoFrameType,
};

// =============================================================================
// Strategy 定義
// =============================================================================

/// Protocol Control Message 用のヘッダを生成する
fn arb_pcm_header() -> impl Strategy<Value = RtmpMessageHeader> {
    any::<u32>().prop_map(|timestamp_ms| RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::PCM,
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    })
}

/// Media Message 用のヘッダを生成する
fn arb_media_header() -> impl Strategy<Value = RtmpMessageHeader> {
    (any::<u32>(), any::<u32>()).prop_map(|(stream_id, timestamp_ms)| RtmpMessageHeader {
        stream_id: RtmpMessageStreamId::new(stream_id),
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
    })
}

/// 有効な RtmpChunkSize を生成する
fn arb_chunk_size() -> impl Strategy<Value = RtmpChunkSize> {
    (1usize..=4096).prop_filter_map("valid chunk size", RtmpChunkSize::new)
}

/// User Control Event を生成する
fn arb_user_control_event() -> impl Strategy<Value = RtmpUserControlEvent> {
    prop_oneof![
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamBegin {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamEof {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        any::<u32>().prop_map(|stream_id| RtmpUserControlEvent::StreamDry {
            stream_id: RtmpMessageStreamId::new(stream_id),
        }),
        any::<u32>().prop_map(|timestamp_ms| RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        }),
        any::<u32>().prop_map(|timestamp_ms| RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        }),
    ]
}

/// AudioFrame を生成する
fn arb_audio_frame() -> impl Strategy<Value = AudioFrame> {
    (
        any::<u32>(),
        prop_oneof![Just(AudioFormat::Mp3), Just(AudioFormat::Aac)],
        prop_oneof![
            Just(AudioSampleRate::Khz5),
            Just(AudioSampleRate::Khz11),
            Just(AudioSampleRate::Khz22),
            Just(AudioSampleRate::Khz44),
        ],
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        vec(any::<u8>(), 0..64),
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

/// VideoFrame を生成する
fn arb_video_frame() -> impl Strategy<Value = VideoFrame> {
    prop_oneof![
        (any::<u32>(), vec(any::<u8>(), 0..64)).prop_map(|(timestamp_ms, data)| VideoFrame {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            composition_timestamp_offset: RtmpTimestampDelta::ZERO,
            frame_type: VideoFrameType::KeyFrame,
            codec: VideoCodec::Jpeg,
            avc_packet_type: None,
            data,
        }),
        (
            any::<u32>(),
            -1000i32..=1000i32,
            prop_oneof![
                Just(AvcPacketType::SequenceHeader),
                Just(AvcPacketType::NalUnit),
                Just(AvcPacketType::EndOfSequence),
            ],
            vec(any::<u8>(), 0..64),
        )
            .prop_map(
                |(timestamp_ms, offset_ms, avc_packet_type, data)| VideoFrame {
                    timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                    composition_timestamp_offset: RtmpTimestampDelta::from_millis(offset_ms),
                    frame_type: VideoFrameType::KeyFrame,
                    codec: VideoCodec::Avc,
                    avc_packet_type: Some(avc_packet_type),
                    data,
                }
            ),
    ]
}

/// 代表的な RTMP Message を生成する
fn arb_message() -> impl Strategy<Value = RtmpMessage> {
    prop_oneof![
        (arb_pcm_header(), arb_chunk_size())
            .prop_map(|(header, size)| { RtmpMessage::SetChunkSize { header, size } }),
        (arb_pcm_header(), any::<u32>()).prop_map(|(header, sequence_number)| {
            RtmpMessage::Ack {
                header,
                sequence_number,
            }
        }),
        (arb_pcm_header(), any::<u32>())
            .prop_map(|(header, size)| RtmpMessage::WinAckSize { header, size }),
        (arb_pcm_header(), arb_user_control_event())
            .prop_map(|(header, event)| { RtmpMessage::UserControl { header, event } }),
        (arb_media_header(), arb_audio_frame()).prop_map(|(header, mut frame)| {
            frame.timestamp = header.timestamp;
            RtmpMessage::Audio { header, frame }
        }),
        (arb_media_header(), arb_video_frame()).prop_map(|(header, mut frame)| {
            frame.timestamp = header.timestamp;
            if frame.avc_packet_type.is_none() {
                frame.composition_timestamp_offset = RtmpTimestampDelta::ZERO;
            }
            RtmpMessage::Video { header, frame }
        }),
    ]
}

// =============================================================================
// Roundtrip / ACK テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// RtmpMessageChannel の送信バッファが decode で再現できることを検証
    #[test]
    fn message_channel_send_roundtrip(messages in vec(arb_message(), 1..20)) {
        let mut channel = RtmpMessageChannel::default();
        for message in &messages {
            channel.feed_send_message(message.clone());
        }

        let buf = channel.send_buf().to_vec();
        let mut decoder = RtmpMessageDecoder::default();
        decoder.feed_buf(&buf);

        let mut decoded = Vec::new();
        while let Some(message) = decoder.decode().expect("decode should succeed") {
            decoded.push(message);
        }

        prop_assert_eq!(decoded, messages);
    }

    /// 受信バイト数と peer_ack_window_size に基づく ACK 送信判定を検証
    #[test]
    fn message_channel_recv_ack_threshold(
        messages in vec(arb_message(), 1..10),
        segment_sizes in vec(0usize..=128, 1..40),
        peer_ack_window_size in 1u32..=10_000u32,
    ) {
        let mut sender = RtmpMessageChannel::default();
        for message in &messages {
            sender.feed_send_message(message.clone());
        }
        let buf = sender.send_buf().to_vec();

        let mut receiver = RtmpMessageChannel::default();
        receiver.set_peer_ack_window_size(peer_ack_window_size);

        let mut total_received: u32 = 0;
        let mut last_ack_sent: u32 = 0;
        let mut offset = 0usize;

        for size in segment_sizes {
            if offset >= buf.len() {
                break;
            }

            let remaining = buf.len() - offset;
            let take = size.min(remaining);
            let chunk = &buf[offset..offset + take];
            offset += take;

            let ack = receiver
                .feed_recv_buf(chunk)
                .map_err(|e| TestCaseError::fail(format!("decode failed: {e:?}")))?;

            total_received += take as u32;
            let unacked = total_received - last_ack_sent;
            if unacked > peer_ack_window_size / 2 {
                prop_assert_eq!(ack, Some(total_received));
                last_ack_sent = total_received;
            } else {
                prop_assert_eq!(ack, None);
            }
        }
        // 最終 ACK の値が受信バイト数を超えないことを確認する
        prop_assert!(last_ack_sent <= total_received);
    }

    /// ACK が届かない場合に advance_send_buf が false を返すことを検証
    #[test]
    fn message_channel_send_ack_timeout(
        messages in vec(arb_message(), 1..10),
        local_ack_window_size in 1u32..=1000u32,
    ) {
        let mut channel = RtmpMessageChannel::default();
        channel.set_local_ack_window_size(local_ack_window_size);

        for message in &messages {
            channel.feed_send_message(message.clone());
        }

        let total_buf_len = channel.send_buf().len() as u32;
        let threshold = local_ack_window_size * 2;

        // バッファが閾値より小さい場合はスキップ
        if total_buf_len <= threshold {
            return Ok(());
        }

        // 閾値を超えるまでは true を返す
        let bytes_to_send = (threshold + 1) as usize;
        let result = channel.advance_send_buf(bytes_to_send);

        // ACK を受信していないので false が返る
        prop_assert!(!result, "advance_send_buf should return false when ack not received");
    }

    /// ACK を受信すると advance_send_buf が true を返し続けることを検証
    #[test]
    fn message_channel_send_with_ack(
        messages in vec(arb_message(), 1..10),
        local_ack_window_size in 100u32..=1000u32,
    ) {
        let mut channel = RtmpMessageChannel::default();
        channel.set_local_ack_window_size(local_ack_window_size);

        for message in &messages {
            channel.feed_send_message(message.clone());
        }

        let total_buf_len = channel.send_buf().len();

        // 小さなチャンクで送信し、適宜 ACK を通知
        let chunk_size = 50usize;
        let mut bytes_sent: u32 = 0;
        let mut offset = 0usize;

        while offset < total_buf_len {
            let to_advance = chunk_size.min(total_buf_len - offset);
            let result = channel.advance_send_buf(to_advance);
            prop_assert!(result, "advance_send_buf should return true when ack received");

            bytes_sent += to_advance as u32;
            offset += to_advance;

            // ACK を通知して unacked_bytes をリセット
            channel.notify_ack_received(bytes_sent);
        }
    }
}
