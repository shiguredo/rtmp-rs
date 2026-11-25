//! RTMP Handshake の Property-Based Testing
//!
//! RTMP 仕様 5.2 節のハンドシェイクシーケンスを対象にする。
//! - C0/S0: 1 バイト (RTMP バージョン、常に 3)
//! - C1/S1: 1536 バイト (タイムスタンプ + ゼロ欄 + 乱数欄)
//! - C2/S2: 1536 バイト (エコーしたタイムスタンプ + 読み取り時刻 + エコーした乱数欄)
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

/// エコー対象のバイト位置を、時刻欄と乱数欄からそれぞれ生成する
fn sample_echo_offsets(ctx: &mut TestCaseContext) -> [usize; 2] {
    [
        noprop::sample_usize_in(ctx, 0..4),
        noprop::sample_usize_in(ctx, 8..HANDSHAKE_PACKET_SIZE),
    ]
}

/// 応答の time2 (読み取り時刻) を指定した値で上書きする
///
/// 呼び出し側は、エコー元のゼロ欄と異なる非ゼロの値を渡す。
fn set_read_time(packet: &mut [u8], read_time: u32) {
    // Adobe RTMP 仕様（2012 年 12 月版）5.2.4 節の time2 の位置。
    // 応答の形式が変わる場合は、この位置と各テストの組み立てを見直す。
    packet[4..8].copy_from_slice(&read_time.to_be_bytes());
}

/// S1 のエコー部分と一致しない C2 に対するサーバーのエラー処理を検証する
#[test]
fn invalid_c2_rejected_by_server() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES_C2_S2, |ctx: &mut TestCaseContext| {
        let modification_value = noprop::sample_u8_in(ctx, 1..=255);
        let read_time = noprop::sample_u32_in(ctx, 1..=u32::MAX);

        // 時刻欄と乱数欄のそれぞれについて、片欄だけが異なる C2 を渡す。
        for modification_index in sample_echo_offsets(ctx) {
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

            // 読み取り時刻が非ゼロでも、エコーの不一致だけは拒否されるはず。
            let mut invalid_c2 = s1.to_vec();
            set_read_time(&mut invalid_c2, read_time);
            invalid_c2[modification_index] =
                invalid_c2[modification_index].wrapping_add(modification_value);

            // 不正な C2 を送信する
            let result = server.feed_recv_buf(&invalid_c2);
            assert!(
                result.is_err(),
                "サーバーが S1 と一致しない C2 (位置 {modification_index}) を受理してしまった"
            );
            assert!(
                !server.is_recv_complete(),
                "サーバーが C2 の不一致 (位置 {modification_index}) でハンドシェイクを完了してしまった"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// C1 のエコー部分と一致しない S2 に対するクライアントのエラー処理を検証する
#[test]
fn invalid_s2_rejected_by_client() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES_C2_S2, |ctx: &mut TestCaseContext| {
        let modification_value = noprop::sample_u8_in(ctx, 1..=255);
        let read_time = noprop::sample_u32_in(ctx, 1..=u32::MAX);

        // 時刻欄と乱数欄のそれぞれについて、片欄だけが異なる S2 を渡す。
        for modification_index in sample_echo_offsets(ctx) {
            let mut client = RtmpClientHandshake::new();
            let c0_c1 = client.send_buf().to_vec();

            // C1 を取得する (C0 の後)
            let c1 = &c0_c1[1..];

            // 正しい S0 + S1 を作成する
            let mut s0_s1_s2 = vec![RTMP_VERSION];
            s0_s1_s2.extend_from_slice(&[0u8; HANDSHAKE_PACKET_SIZE]);

            // 読み取り時刻が非ゼロでも、エコーの不一致だけは拒否されるはず。
            let mut invalid_s2 = c1.to_vec();
            set_read_time(&mut invalid_s2, read_time);
            invalid_s2[modification_index] =
                invalid_s2[modification_index].wrapping_add(modification_value);
            s0_s1_s2.extend_from_slice(&invalid_s2);

            // 不正な S0 + S1 + S2 を送信する
            let result = client.feed_recv_buf(&s0_s1_s2);
            assert!(
                result.is_err(),
                "クライアントが C1 と一致しない S2 (位置 {modification_index}) を受理してしまった"
            );
            assert!(
                !client.is_recv_complete(),
                "クライアントが S2 の不一致 (位置 {modification_index}) でハンドシェイクを完了してしまった"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// 分割受信でも、双方が相手の読み取り時刻を受理してハンドシェイクを完了する
#[test]
fn chunked_handshake_with_nonzero_read_times() -> noprop::TestResult {
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

        let mut s0_s1_s2 = server.send_buf().to_vec();
        server.advance_send_buf(s0_s1_s2.len());

        // Adobe RTMP 仕様（2012 年 12 月版）5.2.4 節に従い、S2 と C2 の読み取り時刻を
        // エコー元のゼロ欄と異なる値にする。パケットの形式が変わる場合はこの位置を見直す。
        let s2_start = 1 + HANDSHAKE_PACKET_SIZE;
        let server_read_time = noprop::sample_u32_in(ctx, 1..=u32::MAX);
        set_read_time(&mut s0_s1_s2[s2_start..], server_read_time);
        // 断片を受けるたびに、最後の断片までは完了しないことを確かめる。
        let s0_s1_s2_len = s0_s1_s2.len();
        for (offset, chunk) in s0_s1_s2.chunks(chunk_size).enumerate() {
            client
                .feed_recv_buf(chunk)
                .expect("クライアントは読み取り時刻の異なる分割 S2 を受理するはず");
            if (offset + 1) * chunk_size < s0_s1_s2_len {
                assert!(
                    !client.is_recv_complete(),
                    "クライアントは最後の断片までハンドシェイクを完了してはいけない"
                );
            }
        }
        // 最後の断片を受けた後は完了しているはず。C2 の組み立てより先に確かめる。
        assert!(
            client.is_recv_complete(),
            "クライアントは最後の断片でハンドシェイクを完了する必要がある"
        );

        let mut c2 = client.send_buf().to_vec();
        client.advance_send_buf(c2.len());
        let client_read_time = noprop::sample_u32_in(ctx, 1..=u32::MAX);
        set_read_time(&mut c2, client_read_time);
        let c2_len = c2.len();
        for (offset, chunk) in c2.chunks(chunk_size).enumerate() {
            server
                .feed_recv_buf(chunk)
                .expect("サーバーは読み取り時刻の異なる分割 C2 を受理するはず");
            if (offset + 1) * chunk_size < c2_len {
                assert!(
                    !server.is_recv_complete(),
                    "サーバーは最後の断片までハンドシェイクを完了してはいけない"
                );
            }
        }
        // 最後の断片を受けた後は完了しているはず。
        assert!(
            server.is_recv_complete(),
            "サーバーは最後の断片でハンドシェイクを完了する必要がある"
        );

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
