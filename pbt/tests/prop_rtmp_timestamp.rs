//! RtmpTimestamp / RtmpTimestampDelta の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{RtmpTimestamp, RtmpTimestampDelta};
use std::time::Duration;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// from_millis / as_millis が可逆であることを検証
    #[test]
    fn timestamp_millis_roundtrip(ms in any::<u32>()) {
        let ts = RtmpTimestamp::from_millis(ms);
        prop_assert_eq!(ts.as_millis(), ms);
    }

    /// as_duration が正しい Duration を返すことを検証
    #[test]
    fn timestamp_as_duration(ms in any::<u32>()) {
        let ts = RtmpTimestamp::from_millis(ms);
        let expected = Duration::from_millis(ms as u64);
        prop_assert_eq!(ts.as_duration(), expected);
    }

    /// wrapping_add がオーバーフロー時にラップアラウンドすることを検証
    #[test]
    fn timestamp_wrapping_add(a in any::<u32>(), b in any::<u32>()) {
        let ts_a = RtmpTimestamp::from_millis(a);
        let ts_b = RtmpTimestamp::from_millis(b);
        let result = ts_a.wrapping_add(ts_b);
        prop_assert_eq!(result.as_millis(), a.wrapping_add(b));
    }

    /// checked_sub が正しく動作することを検証
    #[test]
    fn timestamp_checked_sub(a in any::<u32>(), b in any::<u32>()) {
        let ts_a = RtmpTimestamp::from_millis(a);
        let ts_b = RtmpTimestamp::from_millis(b);
        let result = ts_a.checked_sub(ts_b);
        if a >= b {
            prop_assert!(result.is_some());
            prop_assert_eq!(result.expect("result must be some").as_millis(), a - b);
        } else {
            prop_assert!(result.is_none());
        }
    }

    /// ZERO 定数が 0 ミリ秒であることを検証
    #[test]
    fn timestamp_zero_is_zero(_dummy in Just(())) {
        prop_assert_eq!(RtmpTimestamp::ZERO.as_millis(), 0);
    }

    /// RtmpTimestampDelta の from_millis / as_millis が可逆であることを検証
    #[test]
    fn timestamp_delta_millis_roundtrip(ms in any::<i32>()) {
        let delta = RtmpTimestampDelta::from_millis(ms);
        prop_assert_eq!(delta.as_millis(), ms);
    }

    /// RtmpTimestampDelta の ZERO 定数が 0 ミリ秒であることを検証
    #[test]
    fn timestamp_delta_zero_is_zero(_dummy in Just(())) {
        prop_assert_eq!(RtmpTimestampDelta::ZERO.as_millis(), 0);
    }
}
