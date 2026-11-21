//! RTMP ハンドシェイクの決定的なシナリオを検証する単体テスト
//!
//! RTMP 仕様 5.2 節のハンドシェイク手順そのものと、空データの扱いを確認する。
//! 入力値をランダムに変える余地がないため PBT ではなく単体テストとして置く。

use shiguredo_rtmp::rtmp_handshake::{RtmpClientHandshake, RtmpServerHandshake};

/// RTMP のバージョン
const RTMP_VERSION: u8 = 3;

/// ハンドシェイクのパケットサイズ (C1/S1/C2/S2 のそれぞれ)
const HANDSHAKE_PACKET_SIZE: usize = 1536;

#[test]
fn test_complete_handshake_roundtrip() {
    // クライアント側: C0 + C1 を送信する
    let mut client = RtmpClientHandshake::new();
    let c0_c1 = client.send_buf().to_vec();
    client.advance_send_buf(c0_c1.len());

    // C0 + C1 のサイズを検証する
    assert_eq!(
        c0_c1.len(),
        1 + HANDSHAKE_PACKET_SIZE,
        "C0 + C1 のサイズが 1 + 1536 バイトではない"
    );

    // C0 が RTMP バージョン 3 であることを検証する
    assert_eq!(c0_c1[0], RTMP_VERSION, "C0 が RTMP バージョン 3 ではない");

    // サーバー側: C0 + C1 を受信する
    let mut server = RtmpServerHandshake::new();
    server
        .feed_recv_buf(&c0_c1)
        .expect("サーバーが C0 + C1 を受理しなかった");

    // サーバー側: S0 + S1 + S2 を送信する
    let s0_s1_s2 = server.send_buf().to_vec();
    server.advance_send_buf(s0_s1_s2.len());

    // S0 + S1 + S2 のサイズを検証する
    assert_eq!(
        s0_s1_s2.len(),
        1 + HANDSHAKE_PACKET_SIZE * 2,
        "S0 + S1 + S2 のサイズが 1 + 1536 + 1536 バイトではない"
    );

    // S0 が RTMP バージョン 3 であることを検証する
    assert_eq!(
        s0_s1_s2[0], RTMP_VERSION,
        "S0 が RTMP バージョン 3 ではない"
    );

    // クライアント側: S0 + S1 + S2 を受信する
    client
        .feed_recv_buf(&s0_s1_s2)
        .expect("クライアントが S0 + S1 + S2 を受理しなかった");

    // クライアント側: C2 を送信する
    let c2 = client.send_buf().to_vec();
    client.advance_send_buf(c2.len());

    // C2 のサイズを検証する
    assert_eq!(
        c2.len(),
        HANDSHAKE_PACKET_SIZE,
        "C2 のサイズが 1536 バイトではない"
    );

    // サーバー側: C2 を受信する
    server
        .feed_recv_buf(&c2)
        .expect("サーバーが C2 を受理しなかった");

    // 双方でハンドシェイクが完了していることを検証する
    assert!(
        client.is_recv_complete(),
        "クライアントの受信が完了していない"
    );
    assert!(
        client.is_send_complete(),
        "クライアントの送信が完了していない"
    );
    assert!(server.is_recv_complete(), "サーバーの受信が完了していない");
    assert!(server.is_send_complete(), "サーバーの送信が完了していない");
}

#[test]
fn test_empty_data_handled() {
    let mut server = RtmpServerHandshake::new();
    let mut client = RtmpClientHandshake::new();

    // 空データを送信してもエラーにならないことを検証する
    server
        .feed_recv_buf(&[])
        .expect("サーバーが空データを受理しなかった");
    client
        .feed_recv_buf(&[])
        .expect("クライアントが空データを受理しなかった");

    // 空データではハンドシェイクが完了しないことを検証する
    assert!(
        !server.is_recv_complete(),
        "サーバーの受信が完了してしまった"
    );
    assert!(
        !client.is_recv_complete(),
        "クライアントの受信が完了してしまった"
    );
}
