//! RTMP Client Connection の Property-Based Testing

use proptest::collection::vec;
use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    Amf0Value, AmfValue, AmfVersion, AudioFormat, AudioFrame, AudioSampleRate, Pair, RtmpChunkSize,
    RtmpCommand, RtmpConnectionEvent, RtmpConnectionState, RtmpMessage, RtmpMessageDecoder,
    RtmpMessageEncoder, RtmpMessageHeader, RtmpMessageStreamId, RtmpPlayClientConnection,
    RtmpPublishClientConnection, RtmpResultCommand, RtmpServerHandshake, RtmpTimestamp,
    RtmpTimestampDelta, RtmpUrl, RtmpUserControlEvent, SetPeerBandwidthLimitType, TransactionId,
    VideoCodec, VideoFrame, VideoFrameType,
};
use std::str::FromStr;

// =============================================================================
// Strategy 定義
// =============================================================================

/// 小さめの ASCII 文字列を生成する Strategy
fn arb_small_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_.-]{1,20}".prop_map(|s| s.to_string())
}

/// tcUrl 形式の URL を生成する Strategy
fn arb_tc_url() -> impl Strategy<Value = String> {
    (
        prop_oneof![Just("rtmp"), Just("rtmps")],
        arb_small_string(),
        arb_small_string(),
    )
        .prop_map(|(proto, host, app)| format!("{proto}://{host}/{app}"))
}

/// AudioFrame を生成する Strategy
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

/// VideoFrame を生成する Strategy
fn arb_video_frame() -> impl Strategy<Value = VideoFrame> {
    (
        any::<u32>(),
        prop_oneof![
            Just(VideoFrameType::KeyFrame),
            Just(VideoFrameType::InterFrame),
        ],
        vec(any::<u8>(), 0..64),
    )
        .prop_map(|(timestamp_ms, frame_type, data)| VideoFrame {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            composition_timestamp_offset: RtmpTimestampDelta::ZERO,
            frame_type,
            codec: VideoCodec::Jpeg,
            avc_packet_type: None,
            data,
        })
}

// =============================================================================
// ヘルパー関数
// =============================================================================

/// RTMP メッセージをバイト列にエンコードする
fn encode_message(message: RtmpMessage) -> Vec<u8> {
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();
    encoder.encode(
        &mut buf,
        shiguredo_rtmp::tests::RtmpChunkStreamId::new(3).unwrap(),
        message,
    );
    buf
}

/// クライアントが送ったデータに対するサーバーの応答を取得する
fn perform_handshake_client(client_send_buf: &[u8], server: &mut RtmpServerHandshake) -> Vec<u8> {
    server
        .feed_recv_buf(client_send_buf)
        .expect("server should accept client handshake");
    let s0_s1_s2 = server.send_buf().to_vec();
    server.advance_send_buf(s0_s1_s2.len());
    s0_s1_s2
}

/// 指定した TransactionId の _result を生成する
fn server_result_message(transaction_id: TransactionId) -> RtmpMessage {
    let command = RtmpResultCommand {
        transaction_id,
        properties: AmfValue::Amf0(Amf0Value::Null),
        information: AmfValue::Amf0(Amf0Value::Null),
    };
    RtmpCommand::Result(command)
        .into_message(RtmpMessageHeader::PCM)
        .expect("result should encode")
}

/// onStatus (NetStream.Publish.Start) を生成する
fn server_on_status_publish_start() -> RtmpMessage {
    let command =
        RtmpCommand::OnStatus(shiguredo_rtmp::tests::RtmpOnStatusCommand::publish_start());
    command
        .into_message(RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::MEDIA,
            timestamp: RtmpTimestamp::ZERO,
        })
        .expect("onStatus should encode")
}

/// onStatus (NetStream.Play.Start) を生成する
fn server_on_status_play_start() -> RtmpMessage {
    let command = RtmpCommand::OnStatus(shiguredo_rtmp::tests::RtmpOnStatusCommand::play_start());
    command
        .into_message(RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::MEDIA,
            timestamp: RtmpTimestamp::ZERO,
        })
        .expect("onStatus should encode")
}

/// Connection Event をすべて回収する
fn drain_events(client: &mut RtmpPublishClientConnection) -> Vec<RtmpConnectionEvent> {
    let mut events = Vec::new();
    while let Some(event) = client.next_event() {
        events.push(event);
    }
    events
}

/// Play Client の Connection Event をすべて回収する
fn drain_events_play(client: &mut RtmpPlayClientConnection) -> Vec<RtmpConnectionEvent> {
    let mut events = Vec::new();
    while let Some(event) = client.next_event() {
        events.push(event);
    }
    events
}

/// tcUrl と stream_name から RtmpUrl を構築する
fn construct_rtmp_url(tc_url: &str, stream_name: &str) -> RtmpUrl {
    let full_url = format!("{}/{}", tc_url, stream_name);
    RtmpUrl::from_str(&full_url).expect("valid RTMP URL")
}

/// Publish クライアントを Publishing 状態まで遷移させる
fn setup_publishing_client(tc_url: &str, stream_name: &str) -> RtmpPublishClientConnection {
    let url = construct_rtmp_url(tc_url, stream_name);
    let mut client = RtmpPublishClientConnection::new(url);
    let mut server = RtmpServerHandshake::new();

    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());
    let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("client should accept server handshake");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());

    let connect_result = server_result_message(TransactionId::CONNECT);
    client
        .feed_recv_buf(&encode_message(connect_result))
        .expect("connect result should be handled");

    let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
    client
        .feed_recv_buf(&encode_message(create_stream_result))
        .expect("createStream result should be handled");

    // publish の応答は onStatus (NetStream.Publish.Start) で返す
    let publish_on_status = server_on_status_publish_start();
    client
        .feed_recv_buf(&encode_message(publish_on_status))
        .expect("publish onStatus should be handled");

    drain_events(&mut client);
    client
}

/// Play クライアントを Playing 状態まで遷移させる
fn setup_playing_client(tc_url: &str, stream_name: &str) -> RtmpPlayClientConnection {
    let url = construct_rtmp_url(tc_url, stream_name);
    let mut client = RtmpPlayClientConnection::new(url);
    let mut server = RtmpServerHandshake::new();

    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());
    let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("client should accept server handshake");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());

    let connect_result = server_result_message(TransactionId::CONNECT);
    client
        .feed_recv_buf(&encode_message(connect_result))
        .expect("connect result should be handled");

    let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
    client
        .feed_recv_buf(&encode_message(create_stream_result))
        .expect("createStream result should be handled");

    // play の応答は onStatus (NetStream.Play.Start) で返す
    let play_on_status = server_on_status_play_start();
    client
        .feed_recv_buf(&encode_message(play_on_status))
        .expect("play onStatus should be handled");

    drain_events_play(&mut client);
    client
}

/// エラーレスポンスを生成する
fn server_error_message(transaction_id: TransactionId, description: &str) -> RtmpMessage {
    let properties = AmfValue::Amf0(Amf0Value::Object {
        class_name: None,
        entries: vec![
            Pair {
                key: "code".to_string(),
                value: Amf0Value::String("NetStream.Publish.Error".to_string()),
            },
            Pair {
                key: "description".to_string(),
                value: Amf0Value::String(description.to_string()),
            },
        ],
    });
    let command = RtmpResultCommand {
        transaction_id,
        properties,
        information: AmfValue::Amf0(Amf0Value::Null),
    };
    RtmpCommand::Result(command)
        .into_message(RtmpMessageHeader::PCM)
        .expect("error result should encode")
}

// =============================================================================
// Flow テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Publish 接続の一連の遷移が成立することを検証
    #[test]
    fn publish_client_flow(tc_url in arb_tc_url(), stream_name in arb_small_string()) {
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("client should accept server handshake");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client.feed_recv_buf(&connect_buf).expect("connect result should be handled");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream result should be handled");

        // publish の応答は onStatus (NetStream.Publish.Start) で返す
        let publish_on_status = server_on_status_publish_start();
        let publish_buf = encode_message(publish_on_status);
        client
            .feed_recv_buf(&publish_buf)
            .expect("publish onStatus should be handled");

        // Publishing 状態への遷移イベントを確認する
        let mut saw_publishing = false;
        while let Some(event) = client.next_event() {
            if matches!(event, shiguredo_rtmp::tests::RtmpConnectionEvent::StateChanged(shiguredo_rtmp::tests::RtmpConnectionState::Publishing)) {
                saw_publishing = true;
            }
        }
        prop_assert!(saw_publishing);
    }

    /// Play 接続の一連の遷移が成立することを検証
    #[test]
    fn play_client_flow(tc_url in arb_tc_url(), stream_name in arb_small_string()) {
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPlayClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("client should accept server handshake");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client.feed_recv_buf(&connect_buf).expect("connect result should be handled");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream result should be handled");

        // play の応答は onStatus (NetStream.Play.Start) で返す
        let play_on_status = server_on_status_play_start();
        let play_buf = encode_message(play_on_status);
        client
            .feed_recv_buf(&play_buf)
            .expect("play onStatus should be handled");

        // Playing 状態への遷移イベントを確認する
        let mut saw_playing = false;
        while let Some(event) = client.next_event() {
            if matches!(event, shiguredo_rtmp::tests::RtmpConnectionEvent::StateChanged(shiguredo_rtmp::tests::RtmpConnectionState::Playing)) {
                saw_playing = true;
            }
        }
        prop_assert!(saw_playing);
    }

    /// Control メッセージを順番に処理できることを検証
    #[test]
    fn control_message_flow(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
        win_ack_size in any::<u32>(),
        ack_seq in any::<u32>(),
        peer_bandwidth in any::<u32>(),
    ) {
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();
        drain_events(&mut client);

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("client should accept server handshake");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client.feed_recv_buf(&connect_buf).expect("connect result should be handled");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream result should be handled");

        // publish の応答は onStatus (NetStream.Publish.Start) で返す
        let publish_on_status = server_on_status_publish_start();
        client
            .feed_recv_buf(&encode_message(publish_on_status))
            .expect("publish onStatus should be handled");

        let win_ack = RtmpMessage::WinAckSize {
            header: RtmpMessageHeader::PCM,
            size: win_ack_size,
        };
        client
            .feed_recv_buf(&encode_message(win_ack))
            .expect("win ack should be handled");

        let ack = RtmpMessage::Ack {
            header: RtmpMessageHeader::PCM,
            sequence_number: ack_seq,
        };
        client
            .feed_recv_buf(&encode_message(ack))
            .expect("ack should be handled");

        let set_peer_bandwidth = RtmpMessage::SetPeerBandwidth {
            header: RtmpMessageHeader::PCM,
            size: peer_bandwidth,
            limit_type: SetPeerBandwidthLimitType::Hard,
        };
        client
            .feed_recv_buf(&encode_message(set_peer_bandwidth))
            .expect("set peer bandwidth should be handled");

        let user_control = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::SetBufferLength {
                stream_id: shiguredo_rtmp::tests::RtmpMessageStreamId::new(1),
                length: 1000,
            },
        };
        client
            .feed_recv_buf(&encode_message(user_control))
            .expect("user control should be handled");

        let set_chunk_size = RtmpMessage::SetChunkSize {
            header: RtmpMessageHeader::PCM,
            size: RtmpChunkSize::new(128).unwrap(),
        };
        client
            .feed_recv_buf(&encode_message(set_chunk_size))
            .expect("set chunk size should be handled");

        let data_message = RtmpMessage::Data {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            values: vec![AmfValue::Amf0(Amf0Value::Null)],
        };
        client
            .feed_recv_buf(&encode_message(data_message))
            .expect("data should be handled");

        // onFCPublish は Ignore 扱いになるコマンド
        let ignore_command = RtmpMessage::Command {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            name: "onFCPublish".to_string(),
            transaction_id: TransactionId::ON_STATUS,
            object: AmfValue::Amf0(Amf0Value::Null),
            args: vec![],
        };
        client
            .feed_recv_buf(&encode_message(ignore_command))
            .expect("ignore command should be handled");

        // CommandIgnored の発火を確認する
        let events = drain_events(&mut client);
        prop_assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "command ignored event not found"
        );
    }

    /// tcUrl 不正時にエラーになることを検証
    #[test]
    fn invalid_tc_url_rejected(_stream_name in arb_small_string()) {
        // 不正な URL なので RtmpUrl::from_str が失敗することを確認
        let result = RtmpUrl::from_str("invalid");
        prop_assert!(result.is_err());
    }

    /// Publishing 状態でオーディオ・ビデオを送信できることを検証
    #[test]
    fn publish_send_media(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
        audio_frames in vec(arb_audio_frame(), 1..5),
        video_frames in vec(arb_video_frame(), 1..5),
    ) {
        let mut client = setup_publishing_client(&tc_url, &stream_name);

        // オーディオフレームを送信
        for frame in audio_frames {
            client.send_audio(frame).expect("send_audio should succeed");
        }

        // ビデオフレームを送信
        for frame in video_frames {
            client.send_video(frame).expect("send_video should succeed");
        }

        // 送信バッファにデータがあることを確認
        prop_assert!(!client.send_buf().is_empty());
    }

    /// Playing 状態でオーディオ・ビデオを受信できることを検証
    #[test]
    fn play_receive_media(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
        audio_frame in arb_audio_frame(),
        video_frame in arb_video_frame(),
    ) {
        let mut client = setup_playing_client(&tc_url, &stream_name);

        // サーバーからオーディオメッセージを受信
        let audio_message = RtmpMessage::Audio {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: audio_frame.timestamp,
            },
            frame: audio_frame.clone(),
        };
        client
            .feed_recv_buf(&encode_message(audio_message))
            .expect("audio should be received");

        // サーバーからビデオメッセージを受信
        let video_message = RtmpMessage::Video {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: video_frame.timestamp,
            },
            frame: video_frame.clone(),
        };
        client
            .feed_recv_buf(&encode_message(video_message))
            .expect("video should be received");

        // AudioReceived と VideoReceived イベントがあることを確認
        let events = drain_events_play(&mut client);
        prop_assert!(
            events.iter().any(|event| matches!(event, RtmpConnectionEvent::AudioReceived(_))),
            "AudioReceived event not found"
        );
        prop_assert!(
            events.iter().any(|event| matches!(event, RtmpConnectionEvent::VideoReceived(_))),
            "VideoReceived event not found"
        );
    }

    /// エラーレスポンスを受信すると Disconnecting に遷移することを検証
    #[test]
    fn error_response_causes_disconnect(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
        error_desc in arb_small_string(),
    ) {
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("client should accept server handshake");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        // エラーレスポンスを送信
        let error_message = server_error_message(TransactionId::CONNECT, &error_desc);
        client
            .feed_recv_buf(&encode_message(error_message))
            .expect("error response should be handled");

        // DisconnectedByPeer と Disconnecting への遷移を確認
        let events = drain_events(&mut client);
        prop_assert!(
            events.iter().any(|event| matches!(event, RtmpConnectionEvent::DisconnectedByPeer { .. })),
            "DisconnectedByPeer event not found"
        );
        prop_assert!(
            events.iter().any(|event| matches!(event, RtmpConnectionEvent::StateChanged(RtmpConnectionState::Disconnecting))),
            "Disconnecting state not found"
        );
    }

    /// PingRequest を受信すると PingResponse が送信されることを検証
    #[test]
    fn ping_request_response(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
        ping_timestamp in any::<u32>(),
    ) {
        let mut client = setup_publishing_client(&tc_url, &stream_name);

        // PingRequest を受信
        let ping_request = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::PingRequest {
                timestamp: RtmpTimestamp::from_millis(ping_timestamp),
            },
        };
        client
            .feed_recv_buf(&encode_message(ping_request))
            .expect("ping request should be handled");

        // 送信バッファ全体をデコードして PingResponse を探す
        let send_buf = client.send_buf().to_vec();
        let mut decoder = RtmpMessageDecoder::default();
        decoder.feed_buf(&send_buf);

        let mut found_ping_response = false;
        while let Some(message) = decoder.decode().expect("decode should succeed") {
            if let RtmpMessage::UserControl {
                event: RtmpUserControlEvent::PingResponse { timestamp },
                ..
            } = message
            {
                prop_assert_eq!(
                    timestamp.as_millis(),
                    ping_timestamp,
                    "PingResponse timestamp should match PingRequest timestamp"
                );
                found_ping_response = true;
            }
        }
        prop_assert!(found_ping_response, "PingResponse not found in send_buf");
    }

    /// 無視されるべき _result (transaction_id が不明) が CommandIgnored になることを検証
    #[test]
    fn unknown_result_ignored(
        tc_url in arb_tc_url(),
        stream_name in arb_small_string(),
    ) {
        let mut client = setup_publishing_client(&tc_url, &stream_name);

        // 不明な transaction_id を持つ _result を送信
        let unknown_result = server_result_message(TransactionId::from_f64(999.0));
        client
            .feed_recv_buf(&encode_message(unknown_result))
            .expect("unknown result should be handled");

        // CommandIgnored イベントがあることを確認
        let events = drain_events(&mut client);
        prop_assert!(
            events.iter().any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "CommandIgnored event not found for unknown transaction_id"
        );
    }
}
