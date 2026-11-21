//! RTMP Handshake の Property-Based Testing
//!
//! RTMP 仕様 5.2 節のハンドシェイクシーケンスを対象にする。
//! - C0/S0: 1 バイト (RTMP バージョン、常に 3)
//! - C1/S1: 1536 バイト (タイムスタンプ + バージョン + ランダムデータ)
//! - C2/S2: 1536 バイト (C1/S1 のエコー)
//!
//! 仕様に従い、以下をテストする。
//! - 不正なバージョンに対するエラー処理
//! - 部分的なデータに対する耐性
//! - C2/S2 のエコー不一致に対するエラー処理
//! - データを分割して送信した場合の処理

use noprop::TestCaseContext;
use shiguredo_rtmp::rtmp_handshake::{RtmpClientHandshake, RtmpServerHandshake};

/// RTMP のバージョン
const RTMP_VERSION: u8 = 3;

/// ハンドシェイクのパケットサイズ (C1/S1/C2/S2 のそれぞれ)
const HANDSHAKE_PACKET_SIZE: usize = 1536;

/// このファイルの PBT ケース数
const CASES: usize = 100;

/// C2/S2 検証テストの PBT ケース数
const CASES_C2_S2: usize = 50;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// 不正な RTMP バージョンを生成する
fn sample_invalid_version(ctx: &mut TestCaseContext) -> u8 {
    noprop::sample_with_rejection(ctx, 8, |ctx| {
        let version = noprop::sample_u8(ctx);
        (version != RTMP_VERSION).then_some(version)
    })
}

/// 不正なバージョンに対するサーバーのエラー処理を検証する
#[test]
fn invalid_version_rejected_by_server() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let version = sample_invalid_version(ctx);
        let mut server = RtmpServerHandshake::new();

        // 不正なバージョンの C0 を送信する
        let result = server.feed_recv_buf(&[version]);
        assert!(
            result.is_err(),
            "サーバーが不正な RTMP バージョン {version} を受理してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// 不正なバージョンに対するクライアントのエラー処理を検証する
#[test]
fn invalid_version_rejected_by_client() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let version = sample_invalid_version(ctx);
        let mut client = RtmpClientHandshake::new();

        // 不正なバージョンの S0 を送信する
        let result = client.feed_recv_buf(&[version]);
        assert!(
            result.is_err(),
            "クライアントが不正な RTMP バージョン {version} を受理してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// 部分的な C1 データに対するサーバーの耐性を検証する
///
/// 仕様では、データが揃うまで待機し、エラーにならない。
#[test]
fn server_handles_partial_c1() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let partial_size = noprop::sample_usize_in(ctx, 1..HANDSHAKE_PACKET_SIZE);
        let mut server = RtmpServerHandshake::new();

        // 正しい C0 を送信する
        server
            .feed_recv_buf(&[RTMP_VERSION])
            .expect("正しいバージョンの C0 は受理される");

        // 部分的な C1 を送信する
        let partial_c1 = vec![0u8; partial_size];
        server
            .feed_recv_buf(&partial_c1)
            .expect("部分的な C1 はバッファリングされる");

        assert!(
            !server.is_recv_complete(),
            "部分的な C1 ({partial_size} バイト) でハンドシェイクが完了してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// 部分的な S1+S2 データに対するクライアントの耐性を検証する
#[test]
fn client_handles_partial_s1_s2() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let partial_size = noprop::sample_usize_in(ctx, 1..(HANDSHAKE_PACKET_SIZE * 2));
        let mut client = RtmpClientHandshake::new();

        // 正しい S0 を送信する
        client
            .feed_recv_buf(&[RTMP_VERSION])
            .expect("正しいバージョンの S0 は受理される");

        // 部分的な S1+S2 を送信する
        let partial_data = vec![0u8; partial_size];
        client
            .feed_recv_buf(&partial_data)
            .expect("部分的な S1+S2 はバッファリングされる");

        assert!(
            !client.is_recv_complete(),
            "部分的な S1+S2 ({partial_size} バイト) でハンドシェイクが完了してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// S1 と一致しない C2 に対するサーバーのエラー処理を検証する
#[test]
fn invalid_c2_rejected_by_server() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES_C2_S2, |ctx: &mut TestCaseContext| {
        let modification_index = noprop::sample_usize_in(ctx, 0..HANDSHAKE_PACKET_SIZE);
        let modification_value = noprop::sample_u8_in(ctx, 1..=255);

        // 正常なハンドシェイクを途中まで実行する
        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());

        let mut server = RtmpServerHandshake::new();
        server.feed_recv_buf(&c0_c1).expect("C0 と C1 は受理される");

        let s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // S1 を取得する (S0 の後、S2 の前)
        let s1 = &s0_s1_s2[1..1 + HANDSHAKE_PACKET_SIZE];

        // S1 を改変した C2 を作成する (仕様では C2 は S1 のエコー)
        let mut invalid_c2 = s1.to_vec();
        invalid_c2[modification_index] =
            invalid_c2[modification_index].wrapping_add(modification_value);

        // 不正な C2 を送信する
        let result = server.feed_recv_buf(&invalid_c2);
        assert!(
            result.is_err(),
            "サーバーが S1 と一致しない C2 (位置 {modification_index}) を受理してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// C1 と一致しない S2 に対するクライアントのエラー処理を検証する
#[test]
fn invalid_s2_rejected_by_client() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES_C2_S2, |ctx: &mut TestCaseContext| {
        let modification_index = noprop::sample_usize_in(ctx, 0..HANDSHAKE_PACKET_SIZE);
        let modification_value = noprop::sample_u8_in(ctx, 1..=255);

        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();

        // C1 を取得する (C0 の後)
        let c1 = &c0_c1[1..];

        // 正しい S0 + S1 を作成する
        let mut s0_s1_s2 = vec![RTMP_VERSION];
        s0_s1_s2.extend_from_slice(&[0u8; HANDSHAKE_PACKET_SIZE]);

        // C1 を改変した S2 を作成する (仕様では S2 は C1 のエコー)
        let mut invalid_s2 = c1.to_vec();
        invalid_s2[modification_index] =
            invalid_s2[modification_index].wrapping_add(modification_value);
        s0_s1_s2.extend_from_slice(&invalid_s2);

        // 不正な S0 + S1 + S2 を送信する
        let result = client.feed_recv_buf(&s0_s1_s2);
        assert!(
            result.is_err(),
            "クライアントが C1 と一致しない S2 (位置 {modification_index}) を受理してしまった"
        );
        Ok(())
    })?;
    Ok(())
}

/// データを複数回に分けて送信した場合のハンドシェイクを検証する
#[test]
fn chunked_handshake() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES_C2_S2, |ctx: &mut TestCaseContext| {
        let chunk_size = noprop::sample_usize_in(ctx, 1..100);

        let mut client = RtmpClientHandshake::new();
        let c0_c1 = client.send_buf().to_vec();
        client.advance_send_buf(c0_c1.len());

        let mut server = RtmpServerHandshake::new();

        // C0 + C1 を小さなチャンクに分けて送信する
        for chunk in c0_c1.chunks(chunk_size) {
            server
                .feed_recv_buf(chunk)
                .expect("分割したチャンクは受理される");
        }

        let s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // S0 + S1 + S2 を小さなチャンクに分けて送信する
        for chunk in s0_s1_s2.chunks(chunk_size) {
            client
                .feed_recv_buf(chunk)
                .expect("分割したチャンクは受理される");
        }

        let c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());

        // C2 を小さなチャンクに分けて送信する
        for chunk in c2.chunks(chunk_size) {
            server
                .feed_recv_buf(chunk)
                .expect("分割したチャンクは受理される");
        }

        assert!(
            client.is_recv_complete() && client.is_send_complete(),
            "クライアントのハンドシェイクが完了していない"
        );
        assert!(
            server.is_recv_complete() && server.is_send_complete(),
            "サーバーのハンドシェイクが完了していない"
        );
        Ok(())
    })?;
    Ok(())
}
