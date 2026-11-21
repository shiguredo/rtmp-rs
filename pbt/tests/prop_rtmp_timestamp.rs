//! RtmpTimestamp / RtmpTimestampDelta の Property-Based Testing

use std::time::Duration;

use noprop::TestCaseContext;
use shiguredo_rtmp::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};

/// このファイルの PBT ケース数
const CASES: usize = 300;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// from_millis / as_millis が可逆であることを検証する
#[test]
fn timestamp_millis_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let ms = noprop::sample_u32(ctx);
        let ts = RtmpTimestamp::from_millis(ms);
        assert_eq!(ts.as_millis(), ms, "ミリ秒値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// as_duration が正しい Duration を返すことを検証する
#[test]
fn timestamp_as_duration() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let ms = noprop::sample_u32(ctx);
        let ts = RtmpTimestamp::from_millis(ms);
        let expected = Duration::from_millis(u64::from(ms));
        assert_eq!(ts.as_duration(), expected, "Duration が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// wrapping_add がオーバーフロー時にラップアラウンドすることを検証する
#[test]
fn timestamp_wrapping_add() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let a = noprop::sample_u32(ctx);
        let b = noprop::sample_u32(ctx);
        let result = RtmpTimestamp::from_millis(a).wrapping_add(RtmpTimestamp::from_millis(b));
        assert_eq!(
            result.as_millis(),
            a.wrapping_add(b),
            "加算結果が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// checked_sub がアンダーフロー時に None を返すことを検証する
#[test]
fn timestamp_checked_sub() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let a = noprop::sample_u32(ctx);
        let b = noprop::sample_u32(ctx);
        let result = RtmpTimestamp::from_millis(a).checked_sub(RtmpTimestamp::from_millis(b));
        if a >= b {
            let result = result.expect("a >= b なら減算結果は必ず Some になる");
            assert_eq!(result.as_millis(), a - b, "減算結果が一致しない");
        } else {
            assert!(result.is_none(), "a < b なら減算結果は必ず None になる");
        }
        Ok(())
    })?;
    Ok(())
}

/// RtmpTimestampDelta の from_millis / as_millis が可逆であることを検証する
#[test]
fn timestamp_delta_millis_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let ms = noprop::sample_i32(ctx);
        let delta = RtmpTimestampDelta::from_millis(ms);
        assert_eq!(delta.as_millis(), ms, "ミリ秒値が一致しない");
        Ok(())
    })?;
    Ok(())
}
