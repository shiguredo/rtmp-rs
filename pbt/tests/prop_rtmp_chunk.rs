//! RTMP Chunk の Property-Based Testing

use proptest::prelude::*;
use shiguredo_rtmp::tests::{
    RtmpChunk, RtmpChunkDecoder, RtmpChunkEncoder, RtmpChunkStreamId, RtmpMessageStreamId,
    RtmpMessageType, RtmpTimestamp,
};

/// RtmpChunkStreamId の有効範囲 [2, 65599] の値を生成する Strategy
fn arb_chunk_stream_id() -> impl Strategy<Value = RtmpChunkStreamId> {
    prop_oneof![
        // 1 バイトエンコード範囲 (2-63)
        (2u32..=63).prop_map(|id| RtmpChunkStreamId::new(id).unwrap()),
        // 2 バイトエンコード範囲 (64-319)
        (64u32..=319).prop_map(|id| RtmpChunkStreamId::new(id).unwrap()),
        // 3 バイトエンコード範囲 (320-65599)
        (320u32..=65599).prop_map(|id| RtmpChunkStreamId::new(id).unwrap()),
    ]
}

/// RtmpChunkStreamId の境界値を生成する Strategy
fn arb_chunk_stream_id_boundary() -> impl Strategy<Value = RtmpChunkStreamId> {
    prop_oneof![
        Just(RtmpChunkStreamId::new(2).unwrap()),     // MIN
        Just(RtmpChunkStreamId::new(63).unwrap()),    // 1 バイト上限
        Just(RtmpChunkStreamId::new(64).unwrap()),    // 2 バイト下限
        Just(RtmpChunkStreamId::new(319).unwrap()),   // 2 バイト上限
        Just(RtmpChunkStreamId::new(320).unwrap()),   // 3 バイト下限
        Just(RtmpChunkStreamId::new(65599).unwrap()), // MAX
    ]
}

/// RtmpMessageType を生成する Strategy
fn arb_message_type() -> impl Strategy<Value = RtmpMessageType> {
    prop_oneof![
        Just(RtmpMessageType::SetChunkSize),
        Just(RtmpMessageType::Abort),
        Just(RtmpMessageType::Ack),
        Just(RtmpMessageType::UserControl),
        Just(RtmpMessageType::WinAckSize),
        Just(RtmpMessageType::SetPeerBandwidth),
        Just(RtmpMessageType::Audio),
        Just(RtmpMessageType::Video),
        Just(RtmpMessageType::DataAmf3),
        Just(RtmpMessageType::CommandAmf3),
        Just(RtmpMessageType::DataAmf0),
        Just(RtmpMessageType::CommandAmf0),
    ]
}

/// タイムスタンプを生成する Strategy（ミリ秒単位）
fn arb_timestamp() -> impl Strategy<Value = RtmpTimestamp> {
    prop_oneof![
        // 通常タイムスタンプ (0 - 0xFFFFFE)
        (0u32..=0xFFFFFE).prop_map(RtmpTimestamp::from_millis),
        // 拡張タイムスタンプ境界付近
        (0xFFFFFFu32..=0x1FFFFFFu32).prop_map(RtmpTimestamp::from_millis),
    ]
}

/// タイムスタンプの境界値を生成する Strategy
fn arb_timestamp_boundary() -> impl Strategy<Value = RtmpTimestamp> {
    prop_oneof![
        Just(RtmpTimestamp::ZERO),
        Just(RtmpTimestamp::from_millis(0xFFFFFE)), // 拡張タイムスタンプ直前
        Just(RtmpTimestamp::from_millis(0xFFFFFF)), // 拡張タイムスタンプ境界
        Just(RtmpTimestamp::from_millis(0x1000000)), // 拡張タイムスタンプ
        Just(RtmpTimestamp::from_millis(0x12345678)), // 大きな拡張タイムスタンプ
    ]
}

/// ペイロードを生成する Strategy
fn arb_payload() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        // 空ペイロード
        Just(vec![]),
        // 小さいペイロード (1-127 バイト)
        prop::collection::vec(any::<u8>(), 1..=127),
        // デフォルトチャンクサイズ (128 バイト) 付近
        prop::collection::vec(any::<u8>(), 128..=256),
        // 大きいペイロード (512-1024 バイト)
        prop::collection::vec(any::<u8>(), 512..=1024),
    ]
}

/// RtmpChunk を生成する Strategy
fn arb_chunk() -> impl Strategy<Value = RtmpChunk> {
    (
        arb_chunk_stream_id(),
        any::<u32>(),
        arb_message_type(),
        arb_timestamp(),
        arb_payload(),
    )
        .prop_map(
            |(chunk_stream_id, message_stream_id, message_type, timestamp, payload)| RtmpChunk {
                chunk_stream_id,
                message_stream_id: RtmpMessageStreamId::new(message_stream_id),
                message_type,
                timestamp,
                payload,
            },
        )
}

/// 境界値を重視した RtmpChunk を生成する Strategy
fn arb_chunk_boundary() -> impl Strategy<Value = RtmpChunk> {
    (
        arb_chunk_stream_id_boundary(),
        any::<u32>(),
        arb_message_type(),
        arb_timestamp_boundary(),
        arb_payload(),
    )
        .prop_map(
            |(chunk_stream_id, message_stream_id, message_type, timestamp, payload)| RtmpChunk {
                chunk_stream_id,
                message_stream_id: RtmpMessageStreamId::new(message_stream_id),
                message_type,
                timestamp,
                payload,
            },
        )
}

/// エンコードされたバッファから1つのメッセージを完全にデコードする
fn decode_one_message(decoder: &mut RtmpChunkDecoder, buf: &[u8]) -> (usize, Option<RtmpChunk>) {
    let mut total_consumed = 0;
    let mut remaining = buf;

    loop {
        let (size, maybe_chunk) = decoder.decode(remaining).expect("decode should succeed");
        total_consumed += size;
        remaining = &remaining[size..];

        if let Some(chunk) = maybe_chunk {
            return (total_consumed, Some(chunk));
        }

        if remaining.is_empty() {
            return (total_consumed, None);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Roundtrip: decode(encode(chunk)) == chunk
    #[test]
    fn chunk_roundtrip(chunk in arb_chunk()) {
        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        let mut decoder = RtmpChunkDecoder::default();
        let (size, decoded) = decode_one_message(&mut decoder, &buf);

        prop_assert!(decoded.is_some(), "decoded chunk should not be None");
        let decoded = decoded.unwrap();

        prop_assert_eq!(&decoded, &chunk, "roundtrip should preserve chunk data");
        prop_assert_eq!(size, buf.len(), "decoder should consume all bytes");
    }

    /// 境界値の Roundtrip
    #[test]
    fn chunk_boundary_roundtrip(chunk in arb_chunk_boundary()) {
        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        let mut decoder = RtmpChunkDecoder::default();
        let (size, decoded) = decode_one_message(&mut decoder, &buf);

        prop_assert!(decoded.is_some(), "decoded chunk should not be None");
        let decoded = decoded.unwrap();

        prop_assert_eq!(&decoded, &chunk, "roundtrip should preserve chunk data");
        prop_assert_eq!(size, buf.len(), "decoder should consume all bytes");
    }

    /// チャンクストリーム ID のエンコードサイズが正しいことを検証
    #[test]
    fn chunk_stream_id_encoding_size(id in 2u32..=65599u32) {
        let chunk_stream_id = RtmpChunkStreamId::new(id).unwrap();
        let chunk = RtmpChunk {
            chunk_stream_id,
            message_stream_id: RtmpMessageStreamId::PCM,
            message_type: RtmpMessageType::Ack,
            timestamp: RtmpTimestamp::ZERO,
            payload: vec![],
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        // Basic Header のサイズを検証
        // Format 0 の Message Header は 11 バイト固定
        let expected_basic_header_size = if id < 64 {
            1
        } else if id < 320 {
            2
        } else {
            3
        };

        let expected_total_size = expected_basic_header_size + 11; // Basic Header + Message Header (F0)
        prop_assert_eq!(buf.len(), expected_total_size,
            "chunk stream id {} should use {} byte basic header",
            id, expected_basic_header_size);
    }

    /// 拡張タイムスタンプのエンコードを検証
    #[test]
    fn extended_timestamp_encoding(timestamp_ms in 0u32..=0x2000000u32) {
        let timestamp = RtmpTimestamp::from_millis(timestamp_ms);
        let chunk = RtmpChunk {
            chunk_stream_id: RtmpChunkStreamId::new(4).unwrap(),
            message_stream_id: RtmpMessageStreamId::PCM,
            message_type: RtmpMessageType::Ack,
            timestamp,
            payload: vec![],
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        // 拡張タイムスタンプが必要かどうかで期待サイズが変わる
        let uses_extended = timestamp_ms >= 0xFFFFFF;
        let expected_size = if uses_extended {
            1 + 11 + 4 // Basic Header + Message Header (F0) + Extended Timestamp
        } else {
            1 + 11 // Basic Header + Message Header (F0)
        };

        prop_assert_eq!(buf.len(), expected_size,
            "timestamp {} should {}use extended timestamp",
            timestamp_ms, if uses_extended { "" } else { "not " });
    }

    /// 連続したチャンクの Roundtrip
    #[test]
    fn consecutive_chunks_roundtrip(
        chunk1 in arb_chunk(),
        chunk2_timestamp_delta in 0u32..1000u32,
        chunk2_payload in arb_payload(),
    ) {
        // 同じストリームで連続するチャンクを作成
        let chunk2 = RtmpChunk {
            chunk_stream_id: chunk1.chunk_stream_id,
            message_stream_id: chunk1.message_stream_id,
            message_type: chunk1.message_type,
            timestamp: chunk1
                .timestamp
                .wrapping_add(RtmpTimestamp::from_millis(chunk2_timestamp_delta)),
            payload: chunk2_payload,
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk1);
        encoder.encode(&mut buf, &chunk2);

        let mut decoder = RtmpChunkDecoder::default();
        let mut decoded_chunks = Vec::new();
        let mut remaining = buf.as_slice();

        while !remaining.is_empty() {
            let (size, maybe_chunk) = decoder.decode(remaining).expect("decode should succeed");
            remaining = &remaining[size..];
            if let Some(chunk) = maybe_chunk {
                decoded_chunks.push(chunk);
            }
        }

        prop_assert_eq!(decoded_chunks.len(), 2, "should decode exactly 2 chunks");
        prop_assert_eq!(&decoded_chunks[0], &chunk1);
        prop_assert_eq!(&decoded_chunks[1], &chunk2);
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn chunk_stream_id_min() {
        let id = RtmpChunkStreamId::new(2).unwrap();
        assert_eq!(id.get(), 2);
    }

    #[test]
    fn chunk_stream_id_max() {
        let id = RtmpChunkStreamId::new(65599).unwrap();
        assert_eq!(id.get(), 65599);
    }

    #[test]
    fn chunk_stream_id_out_of_range() {
        assert!(RtmpChunkStreamId::new(1).is_none());
        assert!(RtmpChunkStreamId::new(65600).is_none());
    }

    #[test]
    fn empty_payload_roundtrip() {
        let chunk = RtmpChunk {
            chunk_stream_id: RtmpChunkStreamId::new(4).unwrap(),
            message_stream_id: RtmpMessageStreamId::PCM,
            message_type: RtmpMessageType::Ack,
            timestamp: RtmpTimestamp::ZERO,
            payload: vec![],
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        let mut decoder = RtmpChunkDecoder::default();
        let (_, decoded) = decoder.decode(&buf).unwrap();
        assert_eq!(decoded.unwrap(), chunk);
    }
}
