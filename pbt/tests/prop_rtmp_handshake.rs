//! RTMP Handshake の Property-Based Testing
//!
//! RTMP 仕様 Section 5.2: Handshake
//!
//! ハンドシェイクシーケンス:
//! - C0/S0: 1 byte (RTMP version, 常に 3)
//! - C1/S1: 1536 bytes (timestamp + version + random data)
//! - C2/S2: 1536 bytes (C1/S1 のエコー)
//!
//! 仕様に従い、以下をテストする:
//! - 正常なハンドシェイクシーケンス
//! - 不正なバージョンに対するエラー処理
//! - 部分的なデータに対する耐性

use proptest::prelude::*;
use shiguredo_rtmp::tests::{RtmpClientHandshake, RtmpServerHandshake};

// RTMP 仕様定数
const RTMP_VERSION: u8 = 3;
const HANDSHAKE_PACKET_SIZE: usize = 1536;

// =============================================================================
// 正常系テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 正常なクライアント-サーバーハンドシェイクの完了テスト
    #[test]
    fn complete_handshake_roundtrip(_dummy in Just(())) {
        // クライアント側: C0 + C1 を送信
        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());

        // 検証: C0 + C1 のサイズ
        prop_assert_eq!(c0_c1.len(), 1 + HANDSHAKE_PACKET_SIZE,
            "C0 + C1 should be 1 + 1536 = 1537 bytes");

        // 検証: C0 は RTMP version 3
        prop_assert_eq!(c0_c1[0], RTMP_VERSION, "C0 should be RTMP version 3");

        // サーバー側: C0 + C1 を受信
        let mut server = RtmpServerHandshake::new();
        server.feed_recv_buf(&c0_c1).expect("server should accept C0 + C1");

        // サーバー側: S0 + S1 + S2 を送信
        let s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // 検証: S0 + S1 + S2 のサイズ
        prop_assert_eq!(s0_s1_s2.len(), 1 + HANDSHAKE_PACKET_SIZE * 2,
            "S0 + S1 + S2 should be 1 + 1536 + 1536 = 3073 bytes");

        // 検証: S0 は RTMP version 3
        prop_assert_eq!(s0_s1_s2[0], RTMP_VERSION, "S0 should be RTMP version 3");

        // クライアント側: S0 + S1 + S2 を受信
        client.feed_recv_buf(&s0_s1_s2).expect("client should accept S0 + S1 + S2");

        // クライアント側: C2 を送信
        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        // 検証: C2 のサイズ
        prop_assert_eq!(c2.len(), HANDSHAKE_PACKET_SIZE,
            "C2 should be 1536 bytes");

        // サーバー側: C2 を受信
        server.feed_recv_buf(&c2).expect("server should accept C2");

        // 検証: 両側でハンドシェイク完了
        prop_assert!(client.is_recv_complete(), "client should have completed receiving");
        prop_assert!(client.is_send_complete(), "client should have completed sending");
        prop_assert!(server.is_recv_complete(), "server should have completed receiving");
        prop_assert!(server.is_send_complete(), "server should have completed sending");
    }
}

// =============================================================================
// 異常系テスト (不正なバージョン)
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 不正な RTMP バージョンに対するサーバーのエラー処理
    #[test]
    fn invalid_version_rejected_by_server(
        version in (0u8..=255u8).prop_filter("not valid version", |&v| v != RTMP_VERSION)
    ) {
        let mut server = RtmpServerHandshake::new();

        // 不正なバージョンの C0 を送信
        let result = server.feed_recv_buf(&[version]);

        prop_assert!(result.is_err(), "server should reject invalid RTMP version {}", version);
    }

    /// 不正な RTMP バージョンに対するクライアントのエラー処理
    #[test]
    fn invalid_version_rejected_by_client(
        version in (0u8..=255u8).prop_filter("not valid version", |&v| v != RTMP_VERSION)
    ) {
        let mut client = RtmpClientHandshake::new();

        // 不正なバージョンの S0 を送信
        let result = client.feed_recv_buf(&[version]);

        prop_assert!(result.is_err(), "client should reject invalid RTMP version {}", version);
    }
}

// =============================================================================
// 部分データテスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 部分的な C1 データに対するサーバーの耐性テスト
    /// 仕様: データが揃うまで待機し、エラーにならない
    #[test]
    fn server_handles_partial_c1(partial_size in 1usize..HANDSHAKE_PACKET_SIZE) {
        let mut server = RtmpServerHandshake::new();

        // 正しい C0 を送信
        server.feed_recv_buf(&[RTMP_VERSION]).expect("C0 should be accepted");

        // 部分的な C1 を送信
        let partial_c1 = vec![0u8; partial_size];
        server.feed_recv_buf(&partial_c1).expect("partial C1 should be buffered");

        // 検証: まだ完了していない
        prop_assert!(!server.is_recv_complete(),
            "server should not complete with partial C1 ({} bytes)", partial_size);
    }

    /// 部分的な S1+S2 データに対するクライアントの耐性テスト
    #[test]
    fn client_handles_partial_s1_s2(partial_size in 1usize..(HANDSHAKE_PACKET_SIZE * 2)) {
        let mut client = RtmpClientHandshake::new();

        // 正しい S0 を送信
        client.feed_recv_buf(&[RTMP_VERSION]).expect("S0 should be accepted");

        // 部分的な S1+S2 を送信
        let partial_data = vec![0u8; partial_size];
        client.feed_recv_buf(&partial_data).expect("partial S1+S2 should be buffered");

        // 検証: まだ完了していない
        prop_assert!(!client.is_recv_complete(),
            "client should not complete with partial S1+S2 ({} bytes)", partial_size);
    }

    /// 空データに対する耐性テスト
    #[test]
    fn empty_data_handled(_dummy in Just(())) {
        let mut server = RtmpServerHandshake::new();
        let mut client = RtmpClientHandshake::new();

        // 空データを送信してもエラーにならない
        server.feed_recv_buf(&[]).expect("server should handle empty data");
        client.feed_recv_buf(&[]).expect("client should handle empty data");

        // 検証: まだ完了していない
        prop_assert!(!server.is_recv_complete());
        prop_assert!(!client.is_recv_complete());
    }
}

// =============================================================================
// C2/S2 検証テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// 不正な C2 (S1 と一致しない) に対するサーバーのエラー処理
    #[test]
    fn invalid_c2_rejected_by_server(
        modification_index in 0usize..HANDSHAKE_PACKET_SIZE,
        modification_value in 1u8..=255u8
    ) {
        // 正常なハンドシェイクを途中まで実行
        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());

        let mut server = RtmpServerHandshake::new();
        server.feed_recv_buf(&c0_c1).expect("C0 + C1 should be accepted");

        let s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // S1 を取得 (S0 の後、S2 の前)
        let s1 = &s0_s1_s2[1..1 + HANDSHAKE_PACKET_SIZE];

        // S1 を改変した C2 を作成 (仕様: C2 は S1 のエコーであるべき)
        let mut invalid_c2 = s1.to_vec();
        invalid_c2[modification_index] = invalid_c2[modification_index].wrapping_add(modification_value);

        // 不正な C2 を送信
        let result = server.feed_recv_buf(&invalid_c2);

        prop_assert!(result.is_err(),
            "server should reject C2 that doesn't match S1 (modified at index {})",
            modification_index);
    }

    /// 不正な S2 (C1 と一致しない) に対するクライアントのエラー処理
    #[test]
    fn invalid_s2_rejected_by_client(
        modification_index in 0usize..HANDSHAKE_PACKET_SIZE,
        modification_value in 1u8..=255u8
    ) {
        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();

        // C1 を取得 (C0 の後)
        let c1 = &c0_c1[1..];

        // 正しい S0 + S1 を作成
        let mut s0_s1_s2 = vec![RTMP_VERSION]; // S0
        s0_s1_s2.extend_from_slice(&[0u8; HANDSHAKE_PACKET_SIZE]); // S1

        // C1 を改変した S2 を作成 (仕様: S2 は C1 のエコーであるべき)
        let mut invalid_s2 = c1.to_vec();
        invalid_s2[modification_index] = invalid_s2[modification_index].wrapping_add(modification_value);
        s0_s1_s2.extend_from_slice(&invalid_s2);

        // 不正な S0 + S1 + S2 を送信
        let result = client.feed_recv_buf(&s0_s1_s2);

        prop_assert!(result.is_err(),
            "client should reject S2 that doesn't match C1 (modified at index {})",
            modification_index);
    }
}

// =============================================================================
// バイトストリーム分割テスト
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// データを複数回に分けて送信した場合のテスト
    #[test]
    fn chunked_handshake(chunk_size in 1usize..100usize) {
        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());

        let mut server = RtmpServerHandshake::new();

        // C0 + C1 を小さなチャンクに分けて送信
        for chunk in c0_c1.chunks(chunk_size) {
            server.feed_recv_buf(chunk).expect("chunk should be accepted");
        }

        let s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // S0 + S1 + S2 を小さなチャンクに分けて送信
        for chunk in s0_s1_s2.chunks(chunk_size) {
            client.feed_recv_buf(chunk).expect("chunk should be accepted");
        }

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        // C2 を小さなチャンクに分けて送信
        for chunk in c2.chunks(chunk_size) {
            server.feed_recv_buf(chunk).expect("chunk should be accepted");
        }

        // 検証: 両側でハンドシェイク完了
        prop_assert!(client.is_recv_complete() && client.is_send_complete(),
            "client handshake should be complete");
        prop_assert!(server.is_recv_complete() && server.is_send_complete(),
            "server handshake should be complete");
    }
}
