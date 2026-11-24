//! RTMP Server Connection の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::{AmfValue, AmfVersion};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::error::ErrorKind;
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

#[path = "../../tests/helpers/rtmp_ignored_message.rs"]
pub mod rtmp_ignored_message;

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
    encode_message_on(3, message)
}

fn encode_message_on(chunk_stream_id: u32, message: RtmpMessage) -> Vec<u8> {
    let mut encoder = RtmpMessageEncoder::default();
    let mut buf = Vec::new();
    let chunk_stream_id = RtmpChunkStreamId::new(chunk_stream_id)
        .expect("テストで使うチャンクストリーム ID は有効なはず");
    encoder
        .encode(&mut buf, chunk_stream_id, message)
        .expect("生成した RTMP メッセージはエンコードできるはず");
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

mod connect {
    use shiguredo_rtmp::rtmp_connection::RtmpConnectionOptions;
    use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;

    use super::{
        Amf0Value, AmfValue, CASES, ErrorKind, RtmpCommand, RtmpConnectCommand,
        RtmpConnectionEvent, RtmpConnectionState, RtmpCreateStreamCommand, RtmpMessage,
        RtmpMessageHeader, RtmpMessageStreamId, RtmpPlayCommand, RtmpPublishCommand,
        RtmpServerConnection, RtmpTimestamp, RtmpUserControlEvent, SEED_ENV,
        SetPeerBandwidthLimitType, TestCaseContext, TransactionId, clear_send_buf, drain_events,
        encode_message, encode_message_on, perform_handshake, sample_small_string,
        sample_transaction_id, send_command,
    };

    /// 初回の connect を待っているサーバーを作る
    fn connecting_server() -> RtmpServerConnection {
        let mut server = RtmpServerConnection::new();
        perform_handshake(&mut server);
        drain_events(&mut server);
        assert_eq!(
            server.state(),
            RtmpConnectionState::Connecting,
            "ハンドシェイク後は初回の connect を待つはず"
        );
        server
    }

    /// 初回の connect は応答を従来の順序で送り、Connected に遷移する
    #[test]
    fn connect_flow() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
            let mut server = connecting_server();
            let app = format!("live-{}", sample_small_string(ctx));
            let command = RtmpCommand::Connect(RtmpConnectCommand {
                tc_url: format!("rtmp://example.invalid/{app}"),
                app,
                flash_ver: sample_small_string(ctx),
            });
            let wire = encode_message(
                command.into_message(RtmpMessageHeader::PCM)
                    .expect("初回の connect をメッセージに変換できるはず"),
            );
            let split = noprop::sample_usize_in(ctx, 1..wire.len());
            let (first, rest) = wire.split_at(split);
            server.feed_recv_buf(first).expect("初回の connect の途中まで受理するはず");

            // 受信途中で生成された ACK を送信してから、残りを受信する。
            let mut decoder = RtmpMessageDecoder::default();
            assert_eq!(
                decode_responses(&mut decoder, server.send_buf()),
                [RtmpMessage::Ack { header: RtmpMessageHeader::PCM, sequence_number: split as u32 }],
                "初回接続の完了前は受信バイトに対する ACK だけを返すはず"
            );
            clear_send_buf(&mut server);
            assert_eq!(server.state(), RtmpConnectionState::Connecting, "ACK を送っても初回の接続要求を待つはず");
            assert_eq!(server.next_event(), None, "初回接続が完了する前に状態遷移を通知しないはず");

            server.feed_recv_buf(rest).expect("残りの入力を受信して初回の connect を受理するはず");
            assert_eq!(
                server.state(),
                RtmpConnectionState::Connected,
                "初回の connect を受理すると接続済みになるはず"
            );
            assert_eq!(
                drain_events(&mut server),
                [RtmpConnectionEvent::StateChanged(
                    RtmpConnectionState::Connected
                )],
                "初回の接続成功を 1 回だけ通知するはず"
            );

            let mut responses = decode_responses(&mut decoder, server.send_buf());
            // 入力バイト数に応じた ACK は connect 自体の応答列から除く。
            responses.retain(|message| !matches!(message, RtmpMessage::Ack { .. }));
            // Adobe RTMP 仕様（2012 年 12 月版）7.1.7 節と 7.2.1.1 節の接続応答を検証する。
            // Set Chunk Size を含める点と順序は本ライブラリの契約で、応答形式が変わる場合は見直す。
            let options = RtmpConnectionOptions::default();
            let [win_ack, chunk_size, bandwidth, begin, result] = responses.as_slice() else {
                panic!("初回の connect への応答は 5 件のはず: {responses:?}");
            };
            assert_eq!(
                [win_ack, chunk_size, bandwidth, begin],
                [
                    &RtmpMessage::win_ack_size(options.ack_window_size),
                    &RtmpMessage::set_chunk_size(options.chunk_size),
                    &RtmpMessage::SetPeerBandwidth {
                        header: RtmpMessageHeader::PCM,
                        size: options.ack_window_size,
                        limit_type: SetPeerBandwidthLimitType::Hard,
                    },
                    &RtmpMessage::stream_begin(RtmpMessageStreamId::FIRST),
                ],
                "初回の connect に対する制御メッセージの内容と順序を維持するはず"
            );
            assert!(
                matches!(result, RtmpMessage::Command {
                    header, name, transaction_id, object: AmfValue::Amf0(Amf0Value::Object { .. }), args, ..
                } if *header == RtmpMessageHeader::PCM
                        && name == "_result"
                        && *transaction_id == TransactionId::CONNECT
                        && args.first().and_then(|info| info.expect_object_member("code").ok())
                            .and_then(|code| code.expect_str().ok())
                            == Some("NetConnection.Connect.Success")),
                "最後に初回の connect に対応する成功応答を返すはず: {result:?}"
            );
            Ok(())
        })?;
        Ok(())
    }

    /// 接続後の connect の拒否は、保存済みの情報と進行中の状態を変えない
    #[test]
    fn connect_is_rejected_after_connecting() -> noprop::TestResult {
        let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
        noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
            let suffix = sample_small_string(ctx);
            let app = format!("initial-{suffix}");
            let tc_url = format!("rtmp://example.invalid/{app}");
            let [initial, replacement] = [
                RtmpConnectCommand {
                    app: app.clone(),
                    flash_ver: sample_small_string(ctx),
                    tc_url: tc_url.clone(),
                },
                // 初回とは異なる値にし、状態だけでなく保存情報の上書きも検出する。
                RtmpConnectCommand {
                    app: format!("other-{suffix}"),
                    flash_ver: sample_small_string(ctx),
                    tc_url: format!("rtmp://example.invalid/other-{suffix}"),
                },
            ]
            .map(|command| {
                RtmpCommand::Connect(command)
                    .into_message(RtmpMessageHeader::PCM)
                    .expect("接続要求をメッセージに変換できるはず")
            });

            // Adobe RTMP 仕様（2012 年 12 月版）7.2.1.1 節では connect の ID は 1。
            // この必須値が変わる場合は、型変換エラーを検証するこの入力も見直す。
            let mut invalid = replacement.clone();
            let RtmpMessage::Command { transaction_id, .. } = &mut invalid else {
                panic!("接続要求はコマンドメッセージのはず");
            };
            *transaction_id = sample_transaction_id(ctx);
            let [initial, replacement, invalid] =
                [initial, replacement, invalid].map(encode_message);
            // クライアントからの ping にも応答する既存動作で、拒否の前後の処理順を確認する。
            let millis = noprop::sample_u32(ctx);
            let timestamp = RtmpTimestamp::from_millis(millis);
            let next_timestamp = RtmpTimestamp::from_millis(millis ^ 1);
            let ping = |timestamp| {
                encode_protocol_message(RtmpMessage::UserControl {
                    header: RtmpMessageHeader::PCM,
                    event: RtmpUserControlEvent::PingRequest { timestamp },
                })
            };
            let mut mixed = ping(timestamp);
            mixed.extend_from_slice(&replacement);
            mixed.extend(ping(next_timestamp));

            // 配信と再生は別々の接続で検証する。
            let transaction_id = sample_transaction_id(ctx);
            let stream_name = sample_small_string(ctx);
            let cases = [
                (
                    RtmpCommand::Publish(RtmpPublishCommand {
                        transaction_id,
                        stream_name: stream_name.clone(),
                    }),
                    RtmpConnectionEvent::PublishRequested {
                        app: app.clone(),
                        tc_url: tc_url.clone(),
                        stream_name: stream_name.clone(),
                    },
                    RtmpConnectionState::PublishPending,
                    RtmpConnectionState::Publishing,
                ),
                (
                    RtmpCommand::Play(RtmpPlayCommand {
                        transaction_id,
                        stream_name: stream_name.clone(),
                        start: -1.0,
                    }),
                    RtmpConnectionEvent::PlayRequested {
                        app,
                        tc_url,
                        stream_name,
                    },
                    RtmpConnectionState::PlayPending,
                    RtmpConnectionState::Playing,
                ),
            ];
            for (command, requested, pending, active) in cases {
                let mut server = connecting_server();
                server
                    .feed_recv_buf(&initial)
                    .expect("初回の connect を受理するはず");
                drain_events(&mut server);
                let mut decoder = RtmpMessageDecoder::default();
                decode_responses(&mut decoder, server.send_buf());
                clear_send_buf(&mut server);

                // 小さい ACK ウィンドウを先に適用し、次の受信で ACK が必要になるようにする。
                let small_window =
                    encode_protocol_message(RtmpMessage::win_ack_size(replacement.len() as u32));
                server
                    .feed_recv_buf(&small_window)
                    .expect("ACK ウィンドウを受理するはず");
                decode_responses(&mut decoder, server.send_buf());
                clear_send_buf(&mut server);
                let response = assert_connect_rejected(
                    &mut server,
                    RtmpConnectionState::Connected,
                    ErrorKind::InvalidState,
                    &mixed,
                );
                // ACK は全入力を数え、先行する ping の応答も残す。拒否した connect の応答はない。
                let bytes = (initial.len() + small_window.len() + mixed.len()) as u32;
                assert_eq!(
                    decode_responses(&mut decoder, response),
                    [
                        RtmpMessage::Ack {
                            header: RtmpMessageHeader::PCM,
                            sequence_number: bytes
                        },
                        RtmpMessage::UserControl {
                            header: RtmpMessageHeader::PCM,
                            event: RtmpUserControlEvent::PingResponse { timestamp },
                        }
                    ],
                    "connect の拒否時にも必要な ACK と先行する ping の応答を返すはず"
                );
                clear_send_buf(&mut server);
                server
                    .feed_recv_buf(&[])
                    .expect("空入力で受信処理を再開できるはず");
                assert_eq!(
                    decode_responses(&mut decoder, server.send_buf()),
                    [RtmpMessage::UserControl {
                        header: RtmpMessageHeader::PCM,
                        event: RtmpUserControlEvent::PingResponse {
                            timestamp: next_timestamp
                        },
                    }],
                    "入力を再投入せずに後続の ping だけを処理するはず"
                );
                clear_send_buf(&mut server);

                // 残りは通常のウィンドウで、短い入力に ACK が増えない場合を確認する。
                let normal_window = encode_protocol_message(RtmpMessage::win_ack_size(
                    RtmpConnectionOptions::default().ack_window_size,
                ));
                server
                    .feed_recv_buf(&normal_window)
                    .expect("ACK ウィンドウを更新できるはず");
                let response = assert_connect_rejected(
                    &mut server,
                    RtmpConnectionState::Connected,
                    ErrorKind::InvalidData,
                    &invalid,
                );
                assert!(
                    response.is_empty(),
                    "型変換エラーでは connect の応答を追加しないはず"
                );

                send_command(
                    &mut server,
                    RtmpCommand::CreateStream(RtmpCreateStreamCommand {
                        transaction_id: TransactionId::NON_RESERVED_START,
                    }),
                )
                .expect("拒否後もストリームを作成できるはず");
                drain_events(&mut server);
                clear_send_buf(&mut server);
                let message = command
                    .into_message(RtmpMessageHeader {
                        stream_id: RtmpMessageStreamId::MEDIA,
                        timestamp: RtmpTimestamp::ZERO,
                    })
                    .expect("配信・再生要求を変換できるはず");
                server
                    .feed_recv_buf(&encode_message(message))
                    .expect("配信・再生要求を処理できるはず");
                assert_eq!(
                    drain_events(&mut server),
                    [RtmpConnectionEvent::StateChanged(pending), requested],
                    "拒否後の配信・再生要求には初回の接続情報を使うはず"
                );
                server.accept().expect("配信・再生要求を受理できるはず");
                drain_events(&mut server);
                // 受理済みの要求への応答が送信待ちでも、その内容を変えないことを確認する。
                assert!(
                    !server.send_buf().is_empty(),
                    "受理済みの要求には送信待ちの応答があるはず"
                );
                let response = assert_connect_rejected(
                    &mut server,
                    active,
                    ErrorKind::InvalidState,
                    &replacement,
                );
                assert!(
                    response.is_empty(),
                    "配信中・再生中の拒否では応答を追加しないはず"
                );
            }
            Ok(())
        })?;
        Ok(())
    }

    /// 接続拒否時の状態とイベントを検証し、その呼び出しで追加した送信データだけを返す
    #[track_caller]
    fn assert_connect_rejected<'a>(
        server: &'a mut RtmpServerConnection,
        state: RtmpConnectionState,
        kind: ErrorKind,
        buf: &[u8],
    ) -> &'a [u8] {
        assert_eq!(
            server.state(),
            state,
            "connect を拒否する状態になっているはず"
        );
        let send_before = server.send_buf().to_vec();
        let error = server
            .feed_recv_buf(buf)
            .expect_err("接続済みなら connect を拒否するはず");
        assert_eq!(error.kind, kind, "connect を拒否した理由を区別するはず");
        assert_eq!(
            server.state(),
            state,
            "拒否してもサーバーの状態を維持するはず"
        );
        assert_eq!(
            server.next_event(),
            None,
            "拒否した connect ではイベントを追加しないはず"
        );
        server
            .send_buf()
            .strip_prefix(send_before.as_slice())
            .expect("拒否前に送信待ちだった応答を維持するはず")
    }

    /// 制御メッセージを 1 件ずつ既存の履歴に依存しない形式で書き出す
    fn encode_protocol_message(message: RtmpMessage) -> Vec<u8> {
        // Adobe RTMP 仕様（2012 年 12 月版）5.4 節と 6.2 節の制御用 CSID を使う。
        // 制御用ストリームの割り当てが変わる場合は見直す。
        encode_message_on(2, message)
    }

    /// 圧縮履歴を維持したまま送信バッファのメッセージを読み取る
    fn decode_responses(decoder: &mut RtmpMessageDecoder, buf: &[u8]) -> Vec<RtmpMessage> {
        decoder.feed_buf(buf);
        let mut messages = Vec::new();
        while let Some(message) = decoder
            .decode()
            .expect("サーバーの応答をデコードできるはず")
        {
            messages.push(message);
        }
        messages
    }
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

/// Playing 状態で有効なメディアは送信でき、不正な映像はバッファを変更せずに拒否する
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

        // コーデックに必要な AVC パケットタイプの有無だけを逆にする。
        let pending = server.send_buf().to_vec();
        let invalid = VideoFrame {
            avc_packet_type: if video.codec == VideoCodec::Avc {
                None
            } else {
                Some(AvcPacketType::NalUnit)
            },
            ..video.clone()
        };
        let error = server
            .send_video(invalid)
            .expect_err("再生中でも不正な映像フレームは拒否するはず");
        assert_eq!(error.kind, ErrorKind::InvalidInput, "入力エラーを返すはず");
        assert_eq!(
            server.send_buf(),
            pending,
            "不正な映像は送信待ちのバイト列を変更しないはず"
        );

        server
            .send_video(video)
            .expect("有効な映像フレームは失敗後も送信できるはず");
        assert!(
            server.send_buf().len() > pending.len(),
            "有効な映像が送信バッファに追加されるはず"
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

/// サーバーは無視対象を完成順に 1 件ずつ通知し、同じ入力で後続の音声を処理する
#[test]
fn server_notifies_ignored_messages_in_completion_order() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // ハンドシェイクとは別の受信呼び出しで配信を開始した後、音声と無視対象を送る。
        let mut server = RtmpServerConnection::new();
        perform_handshake(&mut server);
        let mut transaction_id = TransactionId::NON_RESERVED_START;
        connect_and_create_stream(
            &mut server,
            "live".to_owned(),
            "FMLE/3.0".to_owned(),
            "rtmp://example.invalid/live".to_owned(),
            transaction_id,
        );
        transaction_id.increment();
        let publish = RtmpCommand::Publish(RtmpPublishCommand {
            transaction_id,
            stream_name: "stream".to_owned(),
        });
        send_command(&mut server, publish).expect("publish を処理できるはず");
        server.accept().expect("publish を受理できるはず");
        drain_events(&mut server);

        for sample in rtmp_ignored_message::sample_ignored_messages(ctx) {
            sample.assert_events(RtmpConnectionState::Publishing, |buf| {
                server
                    .feed_recv_buf(buf)
                    .expect("サーバーは分割入力を受理するはず");
                (server.state(), drain_events(&mut server))
            });
        }
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
