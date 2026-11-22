//! RTMP Client Connection の Property-Based Testing

use std::str::FromStr;

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::{AmfValue, AmfVersion, Pair};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::error::ErrorKind;
use shiguredo_rtmp::media::{
    AudioFormat, AudioFrame, AudioSampleRate, AvcPacketType, VideoCodec, VideoFrame, VideoFrameType,
};
use shiguredo_rtmp::rtmp_chunk::{RtmpChunkSize, RtmpChunkStreamId};
use shiguredo_rtmp::rtmp_client_connection::{
    RtmpPlayClientConnection, RtmpPublishClientConnection,
};
use shiguredo_rtmp::rtmp_command::{
    RtmpCommand, RtmpOnStatusCommand, RtmpResultCommand, TransactionId,
};
use shiguredo_rtmp::rtmp_connection::{RtmpConnectionEvent, RtmpConnectionState};
use shiguredo_rtmp::rtmp_handshake::RtmpServerHandshake;
use shiguredo_rtmp::rtmp_message::{
    RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId, SetPeerBandwidthLimitType,
};
use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;
use shiguredo_rtmp::rtmp_message_encoder::RtmpMessageEncoder;
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};
use shiguredo_rtmp::rtmp_url::RtmpUrl;
use shiguredo_rtmp::rtmp_user_control_event::RtmpUserControlEvent;

/// このファイルの PBT ケース数 (旧テストの 200 ケースを維持する)
const CASES: usize = 200;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

// =============================================================================
// サンプラー
// =============================================================================

/// 小文字・大文字の英数字と `_`、`.`、`-` からなる長さ 1〜20 の ASCII 文字列を生成する
fn sample_small_string(ctx: &mut TestCaseContext) -> String {
    // 対象の文字種と長さの範囲は旧テストの Strategy と同一に保つ
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-";
    let len = noprop::sample_usize_in(ctx, 1..=20);
    (0..len)
        .map(|_| noprop::sample_choice(ctx, CHARS) as char)
        .collect()
}

/// tcUrl 形式 (`<proto>://<host>/<app>`) の URL を生成する
fn sample_tc_url(ctx: &mut TestCaseContext) -> String {
    let proto = noprop::sample_choice(ctx, &["rtmp", "rtmps"]);
    let host = sample_small_string(ctx);
    let app = sample_small_string(ctx);
    format!("{proto}://{host}/{app}")
}

/// AudioFrame を生成する
fn sample_audio_frame(ctx: &mut TestCaseContext) -> AudioFrame {
    let timestamp_ms = noprop::sample_u32(ctx);
    let format = noprop::sample_choice(ctx, &[AudioFormat::Mp3, AudioFormat::Aac]);

    // 4 種類のサンプリングレートを等確率で選ぶ
    let sample_rate = match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1]) {
        0 => AudioSampleRate::Khz5,
        1 => AudioSampleRate::Khz11,
        2 => AudioSampleRate::Khz22,
        _ => AudioSampleRate::Khz44,
    };

    let is_8bit_sample = noprop::sample_bool(ctx);
    let is_stereo = noprop::sample_bool(ctx);
    let is_aac_sequence_header = noprop::sample_bool(ctx);

    // ペイロードは長さ 0〜63 のバイト列とする
    let data_len = noprop::sample_usize_in(ctx, 0..64);
    let data = noprop::sample_bytes_vec(ctx, data_len);

    // AAC 以外のフォーマットでは AAC シーケンスヘッダーにならない
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
}

/// VideoFrame を生成する
fn sample_video_frame(ctx: &mut TestCaseContext) -> VideoFrame {
    let timestamp_ms = noprop::sample_u32(ctx);
    let frame_type =
        noprop::sample_choice(ctx, &[VideoFrameType::KeyFrame, VideoFrameType::InterFrame]);

    // ペイロードは長さ 0〜63 のバイト列とする
    let data_len = noprop::sample_usize_in(ctx, 0..64);
    let data = noprop::sample_bytes_vec(ctx, data_len);

    VideoFrame {
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        composition_timestamp_offset: RtmpTimestampDelta::ZERO,
        frame_type,
        codec: VideoCodec::Jpeg,
        avc_packet_type: None,
        data,
    }
}

// =============================================================================
// ヘルパー関数
// =============================================================================

/// RTMP メッセージをバイト列にエンコードする
fn encode_message(message: RtmpMessage) -> Vec<u8> {
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();
    encoder
        .encode(
            &mut buf,
            RtmpChunkStreamId::new(3).expect("チャンクストリーム ID 3 は常に有効"),
            message,
        )
        .expect("生成した RTMP メッセージはエンコードできるはず");
    buf
}

/// クライアントが送ったデータに対するサーバーの応答を取得する
fn perform_handshake_client(client_send_buf: &[u8], server: &mut RtmpServerHandshake) -> Vec<u8> {
    server
        .feed_recv_buf(client_send_buf)
        .expect("サーバーがクライアントのハンドシェイクを受け付けられなかった");
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
        .expect("_result をメッセージへ変換できなかった")
}

/// onStatus (NetStream.Publish.Start) を生成する
fn server_on_status_publish_start() -> RtmpMessage {
    let command = RtmpCommand::OnStatus(RtmpOnStatusCommand::publish_start());
    command
        .into_message(RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::MEDIA,
            timestamp: RtmpTimestamp::ZERO,
        })
        .expect("publish 開始の onStatus をメッセージへ変換できなかった")
}

/// onStatus (NetStream.Play.Start) を生成する
fn server_on_status_play_start() -> RtmpMessage {
    let command = RtmpCommand::OnStatus(RtmpOnStatusCommand::play_start());
    command
        .into_message(RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::MEDIA,
            timestamp: RtmpTimestamp::ZERO,
        })
        .expect("play 開始の onStatus をメッセージへ変換できなかった")
}

/// Publish Client の Connection Event をすべて回収する
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
    let full_url = format!("{tc_url}/{stream_name}");
    RtmpUrl::from_str(&full_url).expect("サンプリングした URL は有効な RTMP URL であるべき")
}

/// Publish クライアントを Publishing 状態まで遷移させる
fn setup_publishing_client(tc_url: &str, stream_name: &str) -> RtmpPublishClientConnection {
    let url = construct_rtmp_url(tc_url, stream_name);

    // ケースごとに接続を作り直し、毎回同じ手順でハンドシェイクを進める
    let mut client = RtmpPublishClientConnection::new(url);
    let mut server = RtmpServerHandshake::new();

    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());
    let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());

    let connect_result = server_result_message(TransactionId::CONNECT);
    client
        .feed_recv_buf(&encode_message(connect_result))
        .expect("connect の _result を処理できなかった");

    let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
    client
        .feed_recv_buf(&encode_message(create_stream_result))
        .expect("createStream の _result を処理できなかった");

    // publish の応答は onStatus (NetStream.Publish.Start) で返す
    let publish_on_status = server_on_status_publish_start();
    client
        .feed_recv_buf(&encode_message(publish_on_status))
        .expect("publish の onStatus を処理できなかった");

    drain_events(&mut client);
    client
}

/// Play クライアントを Playing 状態まで遷移させる
fn setup_playing_client(tc_url: &str, stream_name: &str) -> RtmpPlayClientConnection {
    let url = construct_rtmp_url(tc_url, stream_name);

    // ケースごとに接続を作り直し、毎回同じ手順でハンドシェイクを進める
    let mut client = RtmpPlayClientConnection::new(url);
    let mut server = RtmpServerHandshake::new();

    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());
    let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());

    let connect_result = server_result_message(TransactionId::CONNECT);
    client
        .feed_recv_buf(&encode_message(connect_result))
        .expect("connect の _result を処理できなかった");

    let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
    client
        .feed_recv_buf(&encode_message(create_stream_result))
        .expect("createStream の _result を処理できなかった");

    // play の応答は onStatus (NetStream.Play.Start) で返す
    let play_on_status = server_on_status_play_start();
    client
        .feed_recv_buf(&encode_message(play_on_status))
        .expect("play の onStatus を処理できなかった");

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
        .expect("エラー応答をメッセージへ変換できなかった")
}

// =============================================================================
// Flow テスト
// =============================================================================

/// Publish 接続の一連の遷移が成立することを検証する
#[test]
fn publish_client_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);

        // ケースごとに接続を作り直し、同じ手順を再現する
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client
            .feed_recv_buf(&connect_buf)
            .expect("connect の _result を処理できなかった");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream の _result を処理できなかった");

        // publish の応答は onStatus (NetStream.Publish.Start) で返す
        let publish_on_status = server_on_status_publish_start();
        let publish_buf = encode_message(publish_on_status);
        client
            .feed_recv_buf(&publish_buf)
            .expect("publish の onStatus を処理できなかった");

        // Publishing 状態への遷移イベントを確認する
        let mut saw_publishing = false;
        while let Some(event) = client.next_event() {
            if matches!(
                event,
                RtmpConnectionEvent::StateChanged(RtmpConnectionState::Publishing)
            ) {
                saw_publishing = true;
            }
        }
        assert!(
            saw_publishing,
            "Publishing 状態への遷移イベントが見つからない"
        );
        Ok(())
    })?;
    Ok(())
}

/// Play 接続の一連の遷移が成立することを検証する
#[test]
fn play_client_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);

        // ケースごとに接続を作り直し、同じ手順を再現する
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPlayClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client
            .feed_recv_buf(&connect_buf)
            .expect("connect の _result を処理できなかった");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream の _result を処理できなかった");

        // play の応答は onStatus (NetStream.Play.Start) で返す
        let play_on_status = server_on_status_play_start();
        let play_buf = encode_message(play_on_status);
        client
            .feed_recv_buf(&play_buf)
            .expect("play の onStatus を処理できなかった");

        // Playing 状態への遷移イベントを確認する
        let mut saw_playing = false;
        while let Some(event) = client.next_event() {
            if matches!(
                event,
                RtmpConnectionEvent::StateChanged(RtmpConnectionState::Playing)
            ) {
                saw_playing = true;
            }
        }
        assert!(saw_playing, "Playing 状態への遷移イベントが見つからない");
        Ok(())
    })?;
    Ok(())
}

/// Control メッセージを順番に処理できることを検証する
#[test]
fn control_message_flow() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);
        let win_ack_size = noprop::sample_u32(ctx);
        let ack_seq = noprop::sample_u32(ctx);
        let peer_bandwidth = noprop::sample_u32(ctx);

        // ケースごとに接続を作り直し、同じ手順を再現する
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();
        drain_events(&mut client);

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        let connect_result = server_result_message(TransactionId::CONNECT);
        let connect_buf = encode_message(connect_result);
        client
            .feed_recv_buf(&connect_buf)
            .expect("connect の _result を処理できなかった");

        let create_stream_result = server_result_message(TransactionId::NON_RESERVED_START);
        let create_stream_buf = encode_message(create_stream_result);
        client
            .feed_recv_buf(&create_stream_buf)
            .expect("createStream の _result を処理できなかった");

        // publish の応答は onStatus (NetStream.Publish.Start) で返す
        let publish_on_status = server_on_status_publish_start();
        client
            .feed_recv_buf(&encode_message(publish_on_status))
            .expect("publish の onStatus を処理できなかった");

        let win_ack = RtmpMessage::WinAckSize {
            header: RtmpMessageHeader::PCM,
            size: win_ack_size,
        };
        client
            .feed_recv_buf(&encode_message(win_ack))
            .expect("WinAckSize を処理できなかった");

        let ack = RtmpMessage::Ack {
            header: RtmpMessageHeader::PCM,
            sequence_number: ack_seq,
        };
        client
            .feed_recv_buf(&encode_message(ack))
            .expect("Ack を処理できなかった");

        let set_peer_bandwidth = RtmpMessage::SetPeerBandwidth {
            header: RtmpMessageHeader::PCM,
            size: peer_bandwidth,
            limit_type: SetPeerBandwidthLimitType::Hard,
        };
        client
            .feed_recv_buf(&encode_message(set_peer_bandwidth))
            .expect("SetPeerBandwidth を処理できなかった");

        let user_control = RtmpMessage::UserControl {
            header: RtmpMessageHeader::PCM,
            event: RtmpUserControlEvent::SetBufferLength {
                stream_id: RtmpMessageStreamId::new(1),
                length: 1000,
            },
        };
        client
            .feed_recv_buf(&encode_message(user_control))
            .expect("UserControl を処理できなかった");

        let set_chunk_size = RtmpMessage::SetChunkSize {
            header: RtmpMessageHeader::PCM,
            size: RtmpChunkSize::new(128).expect("チャンクサイズ 128 は常に有効"),
        };
        client
            .feed_recv_buf(&encode_message(set_chunk_size))
            .expect("SetChunkSize を処理できなかった");

        let data_message = RtmpMessage::Data {
            header: RtmpMessageHeader::PCM,
            amf_version: AmfVersion::Amf0,
            values: vec![AmfValue::Amf0(Amf0Value::Null)],
        };
        client
            .feed_recv_buf(&encode_message(data_message))
            .expect("Data を処理できなかった");

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
            .expect("onFCPublish を処理できなかった");

        // CommandIgnored の発火を確認する
        let events = drain_events(&mut client);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "CommandIgnored イベントが見つからない"
        );
        Ok(())
    })?;
    Ok(())
}

/// Publishing 状態で有効なメディアは送信でき、不正な映像はバッファを変更せずに拒否する
#[test]
fn publish_send_media() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);

        // オーディオフレームを 1〜4 個サンプリングする
        let audio_frame_count = noprop::sample_usize_in(ctx, 1..5);
        let audio_frames: Vec<AudioFrame> = (0..audio_frame_count)
            .map(|_| sample_audio_frame(ctx))
            .collect();

        // ビデオフレームを 1〜4 個サンプリングする
        let video_frame_count = noprop::sample_usize_in(ctx, 1..5);
        let video_frames: Vec<VideoFrame> = (0..video_frame_count)
            .map(|_| sample_video_frame(ctx))
            .collect();

        // ケースごとに Publishing 状態のクライアントを作り直す
        let mut client = setup_publishing_client(&tc_url, &stream_name);

        // オーディオフレームを送信
        for frame in audio_frames {
            client
                .send_audio(frame)
                .expect("AudioFrame の送信に失敗した");
        }

        for frame in video_frames {
            // JPEG フレームに AVC パケットタイプだけを加えて、配信中の入力エラーを検証する。
            let pending = client.send_buf().to_vec();
            let invalid = VideoFrame {
                avc_packet_type: Some(AvcPacketType::NalUnit),
                ..frame.clone()
            };
            let error = client
                .send_video(invalid)
                .expect_err("配信中でも不正な映像フレームは拒否するはず");
            assert_eq!(error.kind, ErrorKind::InvalidInput, "入力エラーを返すはず");
            assert_eq!(
                client.send_buf(),
                pending,
                "不正な映像は送信待ちのバイト列を変更しないはず"
            );

            // 同じフレームを正しい組み合わせで送信すれば、送信バッファに追加される。
            client
                .send_video(frame)
                .expect("有効な映像フレームは失敗後も送信できるはず");
            assert!(
                client.send_buf().len() > pending.len(),
                "有効な映像が送信バッファに追加されるはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// Playing 状態でオーディオ・ビデオを受信できることを検証する
#[test]
fn play_receive_media() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);
        let audio_frame = sample_audio_frame(ctx);
        let video_frame = sample_video_frame(ctx);

        // ケースごとに Playing 状態のクライアントを作り直す
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
            .expect("Audio を受信できなかった");

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
            .expect("Video を受信できなかった");

        // AudioReceived と VideoReceived イベントがあることを確認
        let events = drain_events_play(&mut client);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::AudioReceived(_))),
            "AudioReceived イベントが見つからない"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::VideoReceived(_))),
            "VideoReceived イベントが見つからない"
        );
        Ok(())
    })?;
    Ok(())
}

/// エラーレスポンスを受信すると Disconnecting に遷移することを検証する
#[test]
fn error_response_causes_disconnect() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);
        let error_desc = sample_small_string(ctx);

        // ケースごとに接続を作り直し、同じ手順を再現する
        let url = construct_rtmp_url(&tc_url, &stream_name);
        let mut client = RtmpPublishClientConnection::new(url);
        let mut server = RtmpServerHandshake::new();

        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());
        let s0_s1_s2 = perform_handshake_client(&c0_c1, &mut server);
        client
            .feed_recv_buf(&s0_s1_s2)
            .expect("クライアントがサーバーのハンドシェイクを受け付けられなかった");

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        // エラーレスポンスを送信
        let error_message = server_error_message(TransactionId::CONNECT, &error_desc);
        client
            .feed_recv_buf(&encode_message(error_message))
            .expect("エラー応答を処理できなかった");

        // DisconnectedByPeer と Disconnecting への遷移を確認
        let events = drain_events(&mut client);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::DisconnectedByPeer { .. })),
            "DisconnectedByPeer イベントが見つからない"
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                RtmpConnectionEvent::StateChanged(RtmpConnectionState::Disconnecting)
            )),
            "Disconnecting 状態への遷移が見つからない"
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
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);
        let ping_timestamp = noprop::sample_u32(ctx);

        // ケースごとに Publishing 状態のクライアントを作り直す
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
            .expect("PingRequest を処理できなかった");

        // 送信バッファ全体をデコードして PingResponse を探す
        let send_buf = client.send_buf().to_vec();
        let mut decoder = RtmpMessageDecoder::default();
        decoder.feed_buf(&send_buf);

        let mut found_ping_response = false;
        while let Some(message) = decoder
            .decode()
            .expect("送信バッファのメッセージをデコードできなかった")
        {
            if let RtmpMessage::UserControl {
                event: RtmpUserControlEvent::PingResponse { timestamp },
                ..
            } = message
            {
                assert_eq!(
                    timestamp.as_millis(),
                    ping_timestamp,
                    "PingResponse のタイムスタンプが PingRequest と一致しない"
                );
                found_ping_response = true;
            }
        }
        assert!(
            found_ping_response,
            "送信バッファに PingResponse が見つからない"
        );
        Ok(())
    })?;
    Ok(())
}

/// 無視されるべき _result (transaction_id が不明) が CommandIgnored になることを検証する
#[test]
fn unknown_result_ignored() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let tc_url = sample_tc_url(ctx);
        let stream_name = sample_small_string(ctx);

        // ケースごとに Publishing 状態のクライアントを作り直す
        let mut client = setup_publishing_client(&tc_url, &stream_name);

        // 不明な transaction_id を持つ _result を送信
        let unknown_result = server_result_message(TransactionId::from_f64(999.0));
        client
            .feed_recv_buf(&encode_message(unknown_result))
            .expect("未知の transaction_id の _result を処理できなかった");

        // CommandIgnored イベントがあることを確認
        let events = drain_events(&mut client);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, RtmpConnectionEvent::CommandIgnored { .. })),
            "未知の transaction_id に対する CommandIgnored イベントが見つからない"
        );
        Ok(())
    })?;
    Ok(())
}
