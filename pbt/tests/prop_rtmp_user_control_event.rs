//! User Control Event の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{RtmpMessageStreamId, RtmpTimestamp, RtmpUserControlEvent};

/// RtmpMessageStreamId を生成する
fn arb_stream_id() -> impl Strategy<Value = RtmpMessageStreamId> {
    any::<u32>().prop_map(RtmpMessageStreamId::new)
}

/// UserControlEvent を生成する
fn arb_event() -> impl Strategy<Value = RtmpUserControlEvent> {
    prop_oneof![
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::StreamBegin { stream_id }),
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::StreamEof { stream_id }),
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::StreamDry { stream_id }),
        (arb_stream_id(), any::<u32>()).prop_map(|(stream_id, length)| {
            RtmpUserControlEvent::SetBufferLength { stream_id, length }
        }),
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::StreamIsRecorded { stream_id }),
        any::<u32>().prop_map(|ms| RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(ms),
        }),
        any::<u32>().prop_map(|ms| RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(ms),
        }),
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::BufferEmpty { stream_id }),
        arb_stream_id().prop_map(|stream_id| RtmpUserControlEvent::BufferReady { stream_id }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// UserControlEvent の encode / decode が可逆であることを検証
    #[test]
    fn user_control_roundtrip(event in arb_event()) {
        let mut buf = Vec::new();
        event.encode(&mut buf);
        let decoded = RtmpUserControlEvent::decode(&buf).expect("decode should succeed");
        prop_assert_eq!(decoded, event);
    }
}
