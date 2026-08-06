//! RTMP Server Connection の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    Amf0Value, AmfValue, AmfVersion, AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType,
    RtmpChunkStreamId, RtmpClientHandshake, RtmpCommand, RtmpConnectionEvent, RtmpConnectionState,
    RtmpMessage, RtmpMessageEncoder, RtmpMessageHeader, RtmpMessageStreamId, RtmpServerConnection,
    RtmpTimestamp, RtmpTimestampDelta, RtmpUserControlEvent, SetPeerBandwidthLimitType,
    TransactionId, VideoCodec, VideoFrame, VideoFrameType,
};

// =============================================================================
// Strategy 定義
// =============================================================================

/// 小さめの ASCII 文字列を生成する
fn arb_small_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_./-]{1,20}".prop_map(|s| s.to_string())
}

/// TransactionId を生成する
fn arb_transaction_id() -> impl Strategy<Value = TransactionId> {
    (2i64..=1000i64).prop_map(|v| TransactionId::from_f64(v as f64))
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
        prop::collection::vec(any::<u8>(), 0..64),
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
        (any::<u32>(), prop::collection::vec(any::<u8>(), 0..64)).prop_map(
            |(timestamp_ms, data)| VideoFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                frame_type: VideoFrameType::KeyFrame,
                codec: VideoCodec::Jpeg,
                avc_packet_type: None,
                data,
            },
        ),
        (
            any::<u32>(),
            -1000i32..=1000i32,
            prop_oneof![
                Just(AvcPacketType::SequenceHeader),
                Just(AvcPacketType::NalUnit),
                Just(AvcPacketType::EndOfSequence),
            ],
            prop::collection::vec(any::<u8>(), 0..64),
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

// =============================================================================
// ヘルパー関数
// =============================================================================

/// クライアントハンドシェイクを模擬してサーバーを進める
fn perform_handshake(server: &mut RtmpServerConnection) {
    let mut client = RtmpClientHandshake::new();

    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());
    server
        .feed_recv_buf(&c0_c1)
        .expect("server should accept C0+C1");

    let s0_s1_s2 = server.send_buf().to_vec();
    server.advance_send_buf(s0_s1_s2.len());
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("client should accept S0+S1+S2");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());
    server.feed_recv_buf(&c2).expect("server should accept C2");
}

/// RTMP メッセージをバイト列にエンコードする
fn encode_message(message: RtmpMessage) -> Vec<u8> {
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();
    let chunk_stream_id = RtmpChunkStreamId::new(3).expect("infallible");
    encoder.encode(&mut buf, chunk_stream_id, message);
    buf
}

/// Connection Event をすべて回収する
fn drain_events(server: &mut RtmpServerConnection) -> Vec<RtmpConnectionEvent> {
    let mut events = Vec::new();
    while let Some(event) = server.next_event() {
        events.push(event);
    }
    events
}

/// Command をサーバーに入力する
fn send_command(server: &mut RtmpServerConnection, command: RtmpCommand) -> Result<(), String> {
    let message = command
        .into_message(RtmpMessageHeader::PCM)
        .map_err(|e| format!("encode command failed: {e:?}"))?;
    let buf = encode_message(message);
    server
        .feed_recv_buf(&buf)
        .map_err(|e| format!("feed_recv_buf failed: {e:?}"))?;
    Ok(())
}

/// 状態遷移イベントが含まれるかを判定する
fn expect_state(events: &[RtmpConnectionEvent], state: RtmpConnectionState) -> bool {
    events
        .iter()
        .any(|event| matches!(event, RtmpConnectionEvent::StateChanged(s) if *s == state))
}

/// Connect と CreateStream の一連を送信する
fn connect_and_create_stream(
    server: &mut RtmpServerConnection,
    app: String,
    flash_ver: String,
    tc_url: String,
    transaction_id: TransactionId,
) {
    let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
        app,
        flash_ver,
        tc_url,
    });
    send_command(server, connect).expect("connect should handle");
    drain_events(server);

    let create_stream = RtmpCommand::CreateStream(shiguredo_rtmp::tests::RtmpCreateStreamCommand {
        transaction_id,
    });
    send_command(server, create_stream).expect("createStream should handle");
    drain_events(server);
}

/// サーバー送信バッファを空にする
fn clear_send_buf(server: &mut RtmpServerConnection) {
    let len = server.send_buf().len();
    if len > 0 {
        server.advance_send_buf(len);
    }
}

// =============================================================================
// Flow テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Connect で Connected に遷移することを検証
    #[test]
    fn connect_flow(app in arb_small_string(), flash_ver in arb_small_string(), tc_url in arb_small_string()) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let command = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, command).expect("connect should handle");
        let events = drain_events(&mut server);

        prop_assert!(expect_state(&events, RtmpConnectionState::Connected));
        prop_assert!(!server.send_buf().is_empty());
    }

    /// CreateStream で MediaStreamCreated に遷移することを検証
    #[test]
    fn create_stream_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let create_stream = RtmpCommand::CreateStream(shiguredo_rtmp::tests::RtmpCreateStreamCommand {
            transaction_id,
        });
        send_command(&mut server, create_stream).expect("createStream should handle");
        let events = drain_events(&mut server);

        prop_assert!(expect_state(&events, RtmpConnectionState::MediaStreamCreated));
    }

    /// Publish で Publishing に遷移することを検証
    #[test]
    fn publish_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish should handle");
        let events = drain_events(&mut server);

        prop_assert!(expect_state(&events, RtmpConnectionState::PublishPending));
    }

    /// Play で PlayPending に遷移することを検証
    #[test]
    fn play_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        start in prop::num::f64::NORMAL,
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(shiguredo_rtmp::tests::RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play should handle");
        let events = drain_events(&mut server);

        prop_assert!(expect_state(&events, RtmpConnectionState::PlayPending));
    }

    /// Control メッセージが処理されることを検証
    #[test]
    fn control_message_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        win_ack_size in any::<u32>(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let win_ack = RtmpMessage::WinAckSize {
            header: RtmpMessageHeader::PCM,
            size: win_ack_size,
        };
        let buf = encode_message(win_ack);
        server.feed_recv_buf(&buf).expect("win ack size should handle");

        let user_control = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::SetBufferLength {
                stream_id: RtmpMessageStreamId::new(1),
                length: 1000,
            },
        };
        let buf = encode_message(user_control);
        server.feed_recv_buf(&buf).expect("user control should handle");
    }

    /// accept 後に Publishing へ遷移することを検証
    #[test]
    fn accept_publish_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish should handle");
        drain_events(&mut server);

        server.accept().expect("accept should succeed");
        let events = drain_events(&mut server);
        prop_assert!(expect_state(&events, RtmpConnectionState::Publishing));
        prop_assert!(!server.send_buf().is_empty());
    }

    /// accept 後に Playing へ遷移することを検証
    #[test]
    fn accept_play_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        start in prop::num::f64::NORMAL,
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(shiguredo_rtmp::tests::RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play should handle");
        drain_events(&mut server);

        server.accept().expect("accept should succeed");
        let events = drain_events(&mut server);
        prop_assert!(expect_state(&events, RtmpConnectionState::Playing));
        prop_assert!(!server.send_buf().is_empty());
    }

    /// Playing 状態でメディア送信が可能であることを検証
    #[test]
    fn send_media_in_playing(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        start in prop::num::f64::NORMAL,
        audio in arb_audio_frame(),
        video in arb_video_frame(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(shiguredo_rtmp::tests::RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play should handle");
        drain_events(&mut server);

        server.accept().expect("accept should succeed");
        drain_events(&mut server);

        server.send_audio(audio).expect("send_audio should succeed");
        server.send_video(video).expect("send_video should succeed");
        prop_assert!(!server.send_buf().is_empty());
    }

    /// 受信メディアイベントが通知されることを検証
    #[test]
    fn recv_media_events(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        audio in arb_audio_frame(),
        video in arb_video_frame(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let audio_message = RtmpMessage::Audio {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: audio.timestamp,
            },
            frame: audio.clone(),
        };
        let audio_buf = encode_message(audio_message);
        server.feed_recv_buf(&audio_buf).expect("audio should handle");

        let video_message = RtmpMessage::Video {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: video.timestamp,
            },
            frame: video.clone(),
        };
        let video_buf = encode_message(video_message);
        server.feed_recv_buf(&video_buf).expect("video should handle");

        let events = drain_events(&mut server);
        prop_assert!(events.iter().any(|event| matches!(event, RtmpConnectionEvent::AudioReceived(f) if *f == audio)));
        prop_assert!(events.iter().any(|event| matches!(event, RtmpConnectionEvent::VideoReceived(f) if *f == video)));
    }

    /// SetPeerBandwidth を受信した際に応答が生成されることを検証
    #[test]
    fn set_peer_bandwidth_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        size in any::<u32>(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);
        clear_send_buf(&mut server);

        let message = RtmpMessage::SetPeerBandwidth {
            header: RtmpMessageHeader::PCM,
            size,
            limit_type: SetPeerBandwidthLimitType::Hard,
        };
        let buf = encode_message(message);
        server.feed_recv_buf(&buf).expect("set peer bandwidth should handle");
        prop_assert!(!server.send_buf().is_empty());
    }

    /// deleteStream コマンドを受理できることを検証
    #[test]
    fn delete_stream_command_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_id in any::<u32>(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let delete_stream = RtmpMessage::Command {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            name: "deleteStream".to_string(),
            transaction_id,
            object: AmfValue::Amf0(Amf0Value::Null),
            args: vec![AmfValue::Amf0(Amf0Value::Number(stream_id as f64))],
        };
        let buf = encode_message(delete_stream);
        server.feed_recv_buf(&buf).expect("deleteStream should handle");
    }

    /// 無視対象のコマンドで CommandIgnored が発火することを検証
    #[test]
    fn ignore_command_event(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let message = RtmpMessage::Command {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            name: "releaseStream".to_string(),
            transaction_id,
            object: AmfValue::Amf0(Amf0Value::Null),
            args: vec![],
        };
        let buf = encode_message(message);
        server.feed_recv_buf(&buf).expect("ignore command should handle");

        // CommandIgnored が発生したことを確認する
        let events = drain_events(&mut server);
        prop_assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "command ignored event not found"
        );
    }

    /// Data メッセージを受理できることを検証
    #[test]
    fn data_message_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        _transaction_id in arb_transaction_id(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let message = RtmpMessage::Data {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            values: vec![AmfValue::Amf0(Amf0Value::Null)],
        };
        let buf = encode_message(message);
        server.feed_recv_buf(&buf).expect("data should handle");
    }

    /// GetStreamLength に対する応答が生成されることを検証
    #[test]
    fn get_stream_length_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        let get_stream_length = RtmpCommand::GetStreamLength(
            shiguredo_rtmp::tests::RtmpGetStreamLengthCommand {
                transaction_id,
                stream_name,
            },
        );
        send_command(&mut server, get_stream_length).expect("getStreamLength should handle");
        prop_assert!(!server.send_buf().is_empty());
    }

    /// reject 後に Disconnecting へ遷移することを検証（Publish）
    #[test]
    fn reject_publish_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        reason in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish should handle");
        drain_events(&mut server);

        server.reject(&reason).expect("reject should succeed");
        let events = drain_events(&mut server);
        prop_assert!(expect_state(&events, RtmpConnectionState::Disconnecting));
        prop_assert!(!server.send_buf().is_empty());
    }

    /// reject 後に Disconnecting へ遷移することを検証（Play）
    #[test]
    fn reject_play_flow(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        start in prop::num::f64::NORMAL,
        reason in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(shiguredo_rtmp::tests::RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play should handle");
        drain_events(&mut server);

        server.reject(&reason).expect("reject should succeed");
        let events = drain_events(&mut server);
        prop_assert!(expect_state(&events, RtmpConnectionState::Disconnecting));
        prop_assert!(!server.send_buf().is_empty());
    }

    /// PingRequest を受信すると PingResponse が送信されることを検証
    #[test]
    fn ping_request_response(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        ping_timestamp in any::<u32>(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);
        clear_send_buf(&mut server);

        let ping_request = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::PingRequest {
                timestamp: RtmpTimestamp::from_millis(ping_timestamp),
            },
        };
        let buf = encode_message(ping_request);
        server.feed_recv_buf(&buf).expect("ping request should handle");

        // 送信バッファに PingResponse があることを確認
        prop_assert!(!server.send_buf().is_empty());
    }

    /// PublishPending/PlayPending 以外で accept() を呼ぶとエラーになることを検証
    #[test]
    fn accept_invalid_state_error(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        // Connected 状態で accept() を呼ぶとエラー
        let result = server.accept();
        prop_assert!(result.is_err());
    }

    /// PublishPending/PlayPending 以外で reject() を呼ぶとエラーになることを検証
    #[test]
    fn reject_invalid_state_error(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        reason in arb_small_string(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(shiguredo_rtmp::tests::RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect should handle");
        drain_events(&mut server);

        // Connected 状態で reject() を呼ぶとエラー
        let result = server.reject(&reason);
        prop_assert!(result.is_err());
    }

    /// Playing 以外で send_audio() を呼ぶとエラーになることを検証
    #[test]
    fn send_audio_invalid_state_error(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        audio in arb_audio_frame(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish should handle");
        drain_events(&mut server);

        server.accept().expect("accept should succeed");
        drain_events(&mut server);

        // Publishing 状態で send_audio() を呼ぶとエラー
        let result = server.send_audio(audio);
        prop_assert!(result.is_err());
    }

    /// Playing 以外で send_video() を呼ぶとエラーになることを検証
    #[test]
    fn send_video_invalid_state_error(
        app in arb_small_string(),
        flash_ver in arb_small_string(),
        tc_url in arb_small_string(),
        transaction_id in arb_transaction_id(),
        stream_name in arb_small_string(),
        video in arb_video_frame(),
    ) {
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(shiguredo_rtmp::tests::RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish should handle");
        drain_events(&mut server);

        server.accept().expect("accept should succeed");
        drain_events(&mut server);

        // Publishing 状態で send_video() を呼ぶとエラー
        let result = server.send_video(video);
        prop_assert!(result.is_err());
    }
}
