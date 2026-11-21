//! RTMP Server Connection の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::{AmfValue, AmfVersion};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
};
use shiguredo_rtmp::rtmp_chunk::RtmpChunkStreamId;
use shiguredo_rtmp::rtmp_command::{
    RtmpCommand, RtmpConnectCommand, RtmpCreateStreamCommand, RtmpGetStreamLengthCommand,
    RtmpPlayCommand, RtmpPublishCommand, TransactionId,
};
use shiguredo_rtmp::rtmp_connection::{RtmpConnectionEvent, RtmpConnectionState};
use shiguredo_rtmp::rtmp_handshake::RtmpClientHandshake;
use shiguredo_rtmp::rtmp_message::{
    RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId, SetPeerBandwidthLimitType,
};
use shiguredo_rtmp::rtmp_message_encoder::RtmpMessageEncoder;
use shiguredo_rtmp::rtmp_server_connection::RtmpServerConnection;
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
use shiguredo_rtmp::rtmp_user_control_event::RtmpUserControlEvent;

/// このファイルの PBT ケース数
///
/// 移行前のケース数をそのまま維持する。
const CASES: usize = 200;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

// =============================================================================
// サンプラー定義
// =============================================================================

/// 小さな文字列に使う文字の集合
///
/// 旧実装の `[a-zA-Z0-9_./-]{1,20}` と同じ文字集合に揃えている。
const SMALL_STRING_CHARS: &[u8] =
    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_./-";

/// 小さめの ASCII 文字列をサンプリングする
fn sample_small_string(ctx: &mut TestCaseContext) -> String {
    // 長さは旧実装の `{1, 20}` と同じ範囲から一様に選ぶ
    let len = noprop::sample_usize_in(ctx, 1..=20);
    (0..len)
        .map(|_| noprop::sample_choice(ctx, SMALL_STRING_CHARS) as char)
        .collect()
}

/// TransactionId をサンプリングする
fn sample_transaction_id(ctx: &mut TestCaseContext) -> TransactionId {
    // 予約済みの 0 / 1 を避けた旧実装の `2..=1000` と同じ範囲から一様に選ぶ
    TransactionId::from_f64(noprop::sample_u64_in(ctx, 2..=1000) as f64)
}

/// AudioFrame をサンプリングする
fn sample_audio_frame(ctx: &mut TestCaseContext) -> AudioFrame {
    let timestamp_ms = noprop::sample_u32(ctx);

    // 音声フォーマットは Mp3 と Aac を等確率で選ぶ（旧実装の 2 分岐を維持）
    let format = noprop::sample_choice(ctx, &[AudioFormat::Mp3, AudioFormat::Aac]);

    // サンプリングレートは 4 種類を等確率で選ぶ（旧実装の 4 分岐を維持）
    let sample_rate = noprop::sample_choice(
        ctx,
        &[
            AudioSampleRate::Khz5,
            AudioSampleRate::Khz11,
            AudioSampleRate::Khz22,
            AudioSampleRate::Khz44,
        ],
    );

    let is_8bit_sample = noprop::sample_bool(ctx);
    let is_stereo = noprop::sample_bool(ctx);
    let is_aac_sequence_header = noprop::sample_bool(ctx);

    // ペイロード長は旧実装の `0..64` と同じ範囲から選ぶ
    let data_len = noprop::sample_usize_in(ctx, 0..64);
    let data = noprop::sample_bytes_vec(ctx, data_len);

    // AAC 以外ではシーケンスヘッダーのフラグを立てない（旧実装と同じ正規化）
    let is_aac_sequence_header = format == AudioFormat::Aac && is_aac_sequence_header;

    AudioFrame {
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        format,
        sample_rate,
        is_8bit_sample,
        is_stereo,
        is_aac_sequence_header,
        data,
    }
}

/// VideoFrame をサンプリングする
fn sample_video_frame(ctx: &mut TestCaseContext) -> VideoFrame {
    // JPEG の単一フレームと AVC のパケットを等確率で選ぶ（旧実装の 2 分岐を維持）
    match noprop::sample_weighted_index(ctx, &[1, 1]) {
        0 => {
            let timestamp_ms = noprop::sample_u32(ctx);
            let data_len = noprop::sample_usize_in(ctx, 0..64);
            let data = noprop::sample_bytes_vec(ctx, data_len);
            VideoFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                frame_type: VideoFrameType::KeyFrame,
                codec: VideoCodec::Jpeg,
                avc_packet_type: None,
                data,
            }
        }
        _ => {
            let timestamp_ms = noprop::sample_u32(ctx);
            // 合成時間オフセットは旧実装の `-1000..=1000` と同じ範囲から選ぶ
            let offset_ms = noprop::sample_i32_in(ctx, -1000..=1000);

            // AVC パケットタイプは 3 種類を等確率で選ぶ（旧実装の 3 分岐を維持）
            let avc_packet_type = noprop::sample_choice(
                ctx,
                &[
                    AvcPacketType::SequenceHeader,
                    AvcPacketType::NalUnit,
                    AvcPacketType::EndOfSequence,
                ],
            );

            let data_len = noprop::sample_usize_in(ctx, 0..64);
            let data = noprop::sample_bytes_vec(ctx, data_len);

            VideoFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp_ms),
                composition_timestamp_offset: RtmpTimestampDelta::from_millis(offset_ms),
                frame_type: VideoFrameType::KeyFrame,
                codec: VideoCodec::Avc,
                avc_packet_type: Some(avc_packet_type),
                data,
            }
        }
    }
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
        .expect("サーバーは C0+C1 を受理できるはず");

    let s0_s1_s2 = server.send_buf().to_vec();
    server.advance_send_buf(s0_s1_s2.len());
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("クライアントは S0+S1+S2 を受理できるはず");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());
    server
        .feed_recv_buf(&c2)
        .expect("サーバーは C2 を受理できるはず");
}

/// RTMP メッセージをバイト列にエンコードする
fn encode_message(message: RtmpMessage) -> Vec<u8> {
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();
    let chunk_stream_id = RtmpChunkStreamId::new(3).expect("3 は有効なチャンクストリーム ID");
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
        .map_err(|e| format!("コマンドのエンコードに失敗した: {e:?}"))?;
    let buf = encode_message(message);
    server
        .feed_recv_buf(&buf)
        .map_err(|e| format!("受信バッファへの投入に失敗した: {e:?}"))?;
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
    let connect = RtmpCommand::Connect(RtmpConnectCommand {
        app,
        flash_ver,
        tc_url,
    });
    send_command(server, connect).expect("connect コマンドを処理できるはず");
    drain_events(server);

    let create_stream = RtmpCommand::CreateStream(RtmpCreateStreamCommand { transaction_id });
    send_command(server, create_stream).expect("createStream コマンドを処理できるはず");
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

/// C2 を受信するタイミングにかかわらず、S2 の残りと connect への応答を順に送信する。
#[test]
fn finishes_s2_even_if_c2_arrives_first() -> noprop::TestResult {
    // Adobe RTMP 仕様（2012 年 12 月版）5.2.2～5.2.4 節の単純ハンドシェイクの長さ。
    // その形式が変更された場合は、C2 の生成方法とともに見直す。
    const PACKET_SIZE: usize = 1536;
    const S2_START: usize = 1 + PACKET_SIZE;
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let partial_s2 = noprop::sample_usize_in(ctx, 1..PACKET_SIZE);
        // 既定の C1 はゼロが並ぶため、乱数欄の末尾に値を入れて誤った部分列を区別する。
        // 最後の 1 バイトだけ残っていても、S2 の正しい位置から返されたことが分かる。
        let mut c0_c1 = RtmpClientHandshake::new().send_buf().to_vec();
        c0_c1[S2_START - 1] = noprop::sample_u8_in(ctx, 1..=u8::MAX);
        for s2_sent in [0, partial_s2, PACKET_SIZE] {
            let mut server = RtmpServerConnection::new();
            server
                .feed_recv_buf(&c0_c1)
                .expect("サーバーは C0 と C1 を受理するはず");
            let handshake = server.send_buf().to_vec();
            assert_eq!(handshake.len(), S2_START + PACKET_SIZE);
            assert_eq!(
                handshake.last(),
                c0_c1.last(),
                "S2 の乱数欄末尾は C1 と一致するはず"
            );
            let sent = S2_START + s2_sent;
            server.advance_send_buf(sent);

            // 本クレートのクライアントは S2 も待つため、5.2.1 節で許される早着順を
            // 再現する C2 は実際の S1 から組み立てる。S1 を読んだ時刻は 0 とする。
            let mut c2 = handshake[1..S2_START].to_vec();
            c2[4..8].copy_from_slice(&0_u32.to_be_bytes());
            server.feed_recv_buf(&c2).expect("有効な C2 を受理するはず");
            assert_eq!(server.state(), RtmpConnectionState::Connecting);

            // C2 の処理で受信状態が進んでも、送信側は残っている S2 を返し続ける。
            let remaining = &handshake[sent..];
            assert_eq!(
                server.send_buf(),
                remaining,
                "S2 の未送信部分が一致するはず"
            );
            if !remaining.is_empty() {
                server.advance_send_buf(remaining.len());
            }
            assert!(server.send_buf().is_empty(), "S2 はすべて送信済みのはず");

            // どちらの順序でも、ハンドシェイクの後は応答チャンクを取得・消費できる。
            send_command(
                &mut server,
                RtmpCommand::Connect(RtmpConnectCommand {
                    app: "live".to_owned(),
                    flash_ver: "FMLE/3.0".to_owned(),
                    tc_url: "rtmp://example.invalid/live".to_owned(),
                }),
            )
            .expect("connect を受理するはず");
            assert_eq!(server.state(), RtmpConnectionState::Connected);
            let len = server.send_buf().len();
            assert!(len > 0, "connect の応答チャンクがあるはず");
            server.advance_send_buf(len);
            assert!(server.send_buf().is_empty(), "応答チャンクは送信済みのはず");
        }
        Ok(())
    })?;
    Ok(())
}

/// Connect で Connected に遷移することを検証する
#[test]
fn connect_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        // connect コマンドに載せる値をサンプリングする
        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let command = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, command).expect("connect コマンドを処理できるはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::Connected),
            "Connected 状態への遷移イベントが発火しなかった: {events:?}"
        );
        assert!(
            !server.send_buf().is_empty(),
            "connect コマンドへの応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// CreateStream で MediaStreamCreated に遷移することを検証する
#[test]
fn create_stream_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        let create_stream = RtmpCommand::CreateStream(RtmpCreateStreamCommand { transaction_id });
        send_command(&mut server, create_stream).expect("createStream コマンドを処理できるはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::MediaStreamCreated),
            "MediaStreamCreated 状態への遷移イベントが発火しなかった: {events:?}"
        );
        Ok(())
    })?;
    Ok(())
}

/// Publish で Publishing に遷移することを検証する
#[test]
fn publish_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish コマンドを処理できるはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::PublishPending),
            "PublishPending 状態への遷移イベントが発火しなかった: {events:?}"
        );
        Ok(())
    })?;
    Ok(())
}

/// Play で PlayPending に遷移することを検証する
#[test]
fn play_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let start = noprop::sample_f64(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play コマンドを処理できるはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::PlayPending),
            "PlayPending 状態への遷移イベントが発火しなかった: {events:?}"
        );
        Ok(())
    })?;
    Ok(())
}

/// Control メッセージが処理されることを検証する
#[test]
fn control_message_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let win_ack_size = noprop::sample_u32(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        // ウィンドウサイズの通知を受理できることを確認する
        let win_ack = RtmpMessage::WinAckSize {
            header: RtmpMessageHeader::PCM,
            size: win_ack_size,
        };
        let buf = encode_message(win_ack);
        server
            .feed_recv_buf(&buf)
            .expect("WinAckSize メッセージを処理できるはず");

        // バッファ長の通知を受理できることを確認する
        let user_control = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::SetBufferLength {
                stream_id: RtmpMessageStreamId::new(1),
                length: 1000,
            },
        };
        let buf = encode_message(user_control);
        server
            .feed_recv_buf(&buf)
            .expect("UserControl メッセージを処理できるはず");
        Ok(())
    })?;
    Ok(())
}

/// accept 後に Publishing へ遷移することを検証する
#[test]
fn accept_publish_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish コマンドを処理できるはず");
        drain_events(&mut server);

        server.accept().expect("publish の受理に成功するはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::Publishing),
            "Publishing 状態への遷移イベントが発火しなかった: {events:?}"
        );
        assert!(
            !server.send_buf().is_empty(),
            "accept への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// accept 後に Playing へ遷移することを検証する
#[test]
fn accept_play_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let start = noprop::sample_f64(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play コマンドを処理できるはず");
        drain_events(&mut server);

        server.accept().expect("play の受理に成功するはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::Playing),
            "Playing 状態への遷移イベントが発火しなかった: {events:?}"
        );
        assert!(
            !server.send_buf().is_empty(),
            "accept への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// Playing 状態でメディア送信が可能であることを検証する
#[test]
fn send_media_in_playing() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let start = noprop::sample_f64(ctx);
        let audio = sample_audio_frame(ctx);
        let video = sample_video_frame(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play コマンドを処理できるはず");
        drain_events(&mut server);

        server.accept().expect("play の受理に成功するはず");
        drain_events(&mut server);

        server.send_audio(audio).expect("send_audio に成功するはず");
        server.send_video(video).expect("send_video に成功するはず");
        assert!(
            !server.send_buf().is_empty(),
            "送信した音声・映像が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// 受信メディアイベントが通知されることを検証する
#[test]
fn recv_media_events() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let audio = sample_audio_frame(ctx);
        let video = sample_video_frame(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        // 音声メッセージを送り、イベントとして取り出せることを確認する
        let audio_message = RtmpMessage::Audio {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: audio.timestamp,
            },
            frame: audio.clone(),
        };
        let audio_buf = encode_message(audio_message);
        server
            .feed_recv_buf(&audio_buf)
            .expect("音声メッセージを処理できるはず");

        // 映像メッセージを送り、イベントとして取り出せることを確認する
        let video_message = RtmpMessage::Video {
            header: RtmpMessageHeader {
                stream_id: RtmpMessageStreamId::MEDIA,
                timestamp: video.timestamp,
            },
            frame: video.clone(),
        };
        let video_buf = encode_message(video_message);
        server
            .feed_recv_buf(&video_buf)
            .expect("映像メッセージを処理できるはず");

        let events = drain_events(&mut server);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::AudioReceived(f) if *f == audio)),
            "AudioReceived イベントが発火しなかった: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::VideoReceived(f) if *f == video)),
            "VideoReceived イベントが発火しなかった: {events:?}"
        );
        Ok(())
    })?;
    Ok(())
}

/// SetPeerBandwidth を受信した際に応答が生成されることを検証する
#[test]
fn set_peer_bandwidth_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let size = noprop::sample_u32(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);
        clear_send_buf(&mut server);

        let message = RtmpMessage::SetPeerBandwidth {
            header: RtmpMessageHeader::PCM,
            size,
            limit_type: SetPeerBandwidthLimitType::Hard,
        };
        let buf = encode_message(message);
        server
            .feed_recv_buf(&buf)
            .expect("SetPeerBandwidth メッセージを処理できるはず");
        assert!(
            !server.send_buf().is_empty(),
            "SetPeerBandwidth への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// deleteStream コマンドを受理できることを検証する
#[test]
fn delete_stream_command_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_id = noprop::sample_u32(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
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
        server
            .feed_recv_buf(&buf)
            .expect("deleteStream コマンドを処理できるはず");
        Ok(())
    })?;
    Ok(())
}

/// 無視対象のコマンドで CommandIgnored が発火することを検証する
#[test]
fn ignore_command_event() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        // 明示的に扱わないコマンドを送る
        let message = RtmpMessage::Command {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            name: "releaseStream".to_string(),
            transaction_id,
            object: AmfValue::Amf0(Amf0Value::Null),
            args: vec![],
        };
        let buf = encode_message(message);
        server
            .feed_recv_buf(&buf)
            .expect("無視対象のコマンドを処理できるはず");

        // CommandIgnored が発火したことを確認する
        let events = drain_events(&mut server);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "CommandIgnored イベントが発火しなかった: {events:?}"
        );
        Ok(())
    })?;
    Ok(())
}

/// Data メッセージを受理できることを検証する
#[test]
fn data_message_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        // data メッセージの処理では使わないが、旧実装の入力空間をそのまま保つ
        let _transaction_id = sample_transaction_id(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        let message = RtmpMessage::Data {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            values: vec![AmfValue::Amf0(Amf0Value::Null)],
        };
        let buf = encode_message(message);
        server
            .feed_recv_buf(&buf)
            .expect("data メッセージを処理できるはず");
        Ok(())
    })?;
    Ok(())
}

/// GetStreamLength に対する応答が生成されることを検証する
#[test]
fn get_stream_length_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        let get_stream_length = RtmpCommand::GetStreamLength(RtmpGetStreamLengthCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, get_stream_length)
            .expect("getStreamLength コマンドを処理できるはず");
        assert!(
            !server.send_buf().is_empty(),
            "getStreamLength への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// reject 後に Disconnecting へ遷移することを検証する（Publish）
#[test]
fn reject_publish_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let reason = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish コマンドを処理できるはず");
        drain_events(&mut server);

        server
            .reject(&reason)
            .expect("publish の拒否に成功するはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::Disconnecting),
            "Disconnecting 状態への遷移イベントが発火しなかった: {events:?}"
        );
        assert!(
            !server.send_buf().is_empty(),
            "reject への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// reject 後に Disconnecting へ遷移することを検証する（Play）
#[test]
fn reject_play_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let start = noprop::sample_f64(ctx);
        let reason = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let play = RtmpCommand::Play(RtmpPlayCommand {
            transaction_id,
            stream_name,
            start,
        });
        send_command(&mut server, play).expect("play コマンドを処理できるはず");
        drain_events(&mut server);

        server.reject(&reason).expect("play の拒否に成功するはず");
        let events = drain_events(&mut server);

        assert!(
            expect_state(&events, RtmpConnectionState::Disconnecting),
            "Disconnecting 状態への遷移イベントが発火しなかった: {events:?}"
        );
        assert!(
            !server.send_buf().is_empty(),
            "reject への応答が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// PingRequest を受信すると PingResponse が送信されることを検証する
#[test]
fn ping_request_response() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let ping_timestamp = noprop::sample_u32(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);
        clear_send_buf(&mut server);

        let ping_request = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::PingRequest {
                timestamp: RtmpTimestamp::from_millis(ping_timestamp),
            },
        };
        let buf = encode_message(ping_request);
        server
            .feed_recv_buf(&buf)
            .expect("PingRequest を処理できるはず");

        // 送信バッファに PingResponse が積まれていることを確認する
        assert!(
            !server.send_buf().is_empty(),
            "PingResponse が送信バッファに積まれていない"
        );
        Ok(())
    })?;
    Ok(())
}

/// PublishPending/PlayPending 以外で accept() を呼ぶとエラーになることを検証する
#[test]
fn accept_invalid_state_error() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        // Connected 状態で accept() を呼ぶとエラーになるはず
        let result = server.accept();
        assert!(
            result.is_err(),
            "Connected 状態の accept() がエラーにならなかった"
        );
        Ok(())
    })?;
    Ok(())
}

/// PublishPending/PlayPending 以外で reject() を呼ぶとエラーになることを検証する
#[test]
fn reject_invalid_state_error() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let reason = sample_small_string(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        let connect = RtmpCommand::Connect(RtmpConnectCommand {
            app,
            flash_ver,
            tc_url,
        });
        send_command(&mut server, connect).expect("connect コマンドを処理できるはず");
        drain_events(&mut server);

        // Connected 状態で reject() を呼ぶとエラーになるはず
        let result = server.reject(&reason);
        assert!(
            result.is_err(),
            "Connected 状態の reject() がエラーにならなかった"
        );
        Ok(())
    })?;
    Ok(())
}

/// Playing 以外で send_audio() を呼ぶとエラーになることを検証する
#[test]
fn send_audio_invalid_state_error() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let audio = sample_audio_frame(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish コマンドを処理できるはず");
        drain_events(&mut server);

        server.accept().expect("publish の受理に成功するはず");
        drain_events(&mut server);

        // Publishing 状態で send_audio() を呼ぶとエラーになるはず
        let result = server.send_audio(audio);
        assert!(
            result.is_err(),
            "Publishing 状態の send_audio() がエラーにならなかった"
        );
        Ok(())
    })?;
    Ok(())
}

/// Playing 以外で send_video() を呼ぶとエラーになることを検証する
#[test]
fn send_video_invalid_state_error() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ケースごとに接続を作り直し、前のケースの状態を持ち越さない
        let mut server = RtmpServerConnection::new();
        drain_events(&mut server);

        let app = sample_small_string(ctx);
        let flash_ver = sample_small_string(ctx);
        let tc_url = sample_small_string(ctx);
        let transaction_id = sample_transaction_id(ctx);
        let stream_name = sample_small_string(ctx);
        let video = sample_video_frame(ctx);

        perform_handshake(&mut server);
        drain_events(&mut server);

        connect_and_create_stream(&mut server, app, flash_ver, tc_url, transaction_id);

        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name,
        });
        send_command(&mut server, publish).expect("publish コマンドを処理できるはず");
        drain_events(&mut server);

        server.accept().expect("publish の受理に成功するはず");
        drain_events(&mut server);

        // Publishing 状態で send_video() を呼ぶとエラーになるはず
        let result = server.send_video(video);
        assert!(
            result.is_err(),
            "Publishing 状態の send_video() がエラーにならなかった"
        );
        Ok(())
    })?;
    Ok(())
}
