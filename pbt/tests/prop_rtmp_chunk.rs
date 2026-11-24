//! RTMP Chunk の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::rtmp_chunk::{RtmpChunk, RtmpChunkSize, RtmpChunkStreamId};
use shiguredo_rtmp::rtmp_chunk_decoder::RtmpChunkDecoder;
use shiguredo_rtmp::rtmp_chunk_encoder::RtmpChunkEncoder;
use shiguredo_rtmp::rtmp_message::{RtmpMessageStreamId, RtmpMessageType};
use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;

/// このファイルの PBT ケース数
const CASES: usize = 1000;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// チャンクストリーム ID の有効範囲 [2, 65599] の値を生成する
fn sample_chunk_stream_id(ctx: &mut TestCaseContext) -> RtmpChunkStreamId {
    // 基本ヘッダーのエンコード幅ごとに分岐し、1 バイト・2 バイト・3 バイトの
    // すべての表現を均等に踏むようにする
    let id = match noprop::sample_weighted_index(ctx, &[1, 1, 1]) {
        0 => noprop::sample_u32_in(ctx, 2..=63),
        1 => noprop::sample_u32_in(ctx, 64..=319),
        _ => noprop::sample_u32_in(ctx, 320..=65599),
    };
    RtmpChunkStreamId::new(id).expect("サンプリングした範囲のチャンクストリーム ID は有効")
}

/// チャンクストリーム ID の境界値を生成する
fn sample_chunk_stream_id_boundary(ctx: &mut TestCaseContext) -> RtmpChunkStreamId {
    noprop::sample_choice(
        ctx,
        &[
            RtmpChunkStreamId::new(2).expect("2 はチャンクストリーム ID の最小値"),
            RtmpChunkStreamId::new(63).expect("63 は 1 バイト表現の上限"),
            RtmpChunkStreamId::new(64).expect("64 は 2 バイト表現の下限"),
            RtmpChunkStreamId::new(319).expect("319 は 2 バイト表現の上限"),
            RtmpChunkStreamId::new(320).expect("320 は 3 バイト表現の下限"),
            RtmpChunkStreamId::new(65599).expect("65599 はチャンクストリーム ID の最大値"),
        ],
    )
}

/// メッセージ型を生成する
fn sample_message_type(ctx: &mut TestCaseContext) -> RtmpMessageType {
    noprop::sample_choice(
        ctx,
        &[
            RtmpMessageType::SetChunkSize,
            RtmpMessageType::Abort,
            RtmpMessageType::Ack,
            RtmpMessageType::UserControl,
            RtmpMessageType::WinAckSize,
            RtmpMessageType::SetPeerBandwidth,
            RtmpMessageType::Audio,
            RtmpMessageType::Video,
            RtmpMessageType::DataAmf3,
            RtmpMessageType::SharedObjectAmf3,
            RtmpMessageType::CommandAmf3,
            RtmpMessageType::DataAmf0,
            RtmpMessageType::SharedObjectAmf0,
            RtmpMessageType::CommandAmf0,
            RtmpMessageType::Aggregate,
        ],
    )
}

/// タイムスタンプを生成する
fn sample_timestamp(ctx: &mut TestCaseContext) -> RtmpTimestamp {
    // 通常のタイムスタンプと拡張タイムスタンプの境界付近を半々で生成する
    let millis = match noprop::sample_weighted_index(ctx, &[1, 1]) {
        0 => noprop::sample_u32_in(ctx, 0..=0x00FF_FFFE),
        _ => noprop::sample_u32_in(ctx, 0x00FF_FFFF..=0x01FF_FFFF),
    };
    RtmpTimestamp::from_millis(millis)
}

/// タイムスタンプの境界値を生成する
fn sample_timestamp_boundary(ctx: &mut TestCaseContext) -> RtmpTimestamp {
    noprop::sample_choice(
        ctx,
        &[
            RtmpTimestamp::ZERO,
            RtmpTimestamp::from_millis(0x00FF_FFFE),
            RtmpTimestamp::from_millis(0x00FF_FFFF),
            RtmpTimestamp::from_millis(0x0100_0000),
            RtmpTimestamp::from_millis(0x1234_5678),
        ],
    )
}

/// ペイロードを生成する
fn sample_payload(ctx: &mut TestCaseContext) -> Vec<u8> {
    // 空・小さい・既定チャンクサイズ付近・大きい、の 4 種類を均等に生成する
    let len = match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1]) {
        0 => 0,
        1 => noprop::sample_usize_in(ctx, 1..=127),
        2 => noprop::sample_usize_in(ctx, 128..=256),
        _ => noprop::sample_usize_in(ctx, 512..=1024),
    };
    noprop::sample_bytes_vec(ctx, len)
}

/// 任意のチャンクを生成する
fn sample_chunk(ctx: &mut TestCaseContext) -> RtmpChunk {
    RtmpChunk {
        chunk_stream_id: sample_chunk_stream_id(ctx),
        message_stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        message_type: sample_message_type(ctx),
        timestamp: sample_timestamp(ctx),
        payload: sample_payload(ctx),
    }
}

/// 境界値を重視したチャンクを生成する
fn sample_chunk_boundary(ctx: &mut TestCaseContext) -> RtmpChunk {
    RtmpChunk {
        chunk_stream_id: sample_chunk_stream_id_boundary(ctx),
        message_stream_id: RtmpMessageStreamId::new(noprop::sample_u32(ctx)),
        message_type: sample_message_type(ctx),
        timestamp: sample_timestamp_boundary(ctx),
        payload: sample_payload(ctx),
    }
}

/// エンコードされたバッファから 1 つのチャンクを完全にデコードする
fn decode_one_chunk(decoder: &mut RtmpChunkDecoder, buf: &[u8]) -> (usize, Option<RtmpChunk>) {
    let mut total_consumed = 0;
    let mut remaining = buf;

    loop {
        let (size, maybe_chunk) = decoder
            .decode(remaining)
            .expect("エンコードしたデータのデコードは失敗しない");
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

/// decode(encode(chunk)) が元のチャンクと一致することを検証する
#[test]
fn chunk_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let chunk = sample_chunk(ctx);

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        let mut decoder = RtmpChunkDecoder::default();
        let (size, decoded) = decode_one_chunk(&mut decoder, &buf);

        let decoded = decoded.expect("デコード結果のチャンクが存在しない");
        assert_eq!(decoded, chunk, "ラウンドトリップしたチャンクが一致しない");
        assert_eq!(size, buf.len(), "デコーダがすべてのバイトを消費していない");
        Ok(())
    })?;
    Ok(())
}

/// 境界値のチャンクでも roundtrip が成立することを検証する
#[test]
fn chunk_boundary_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let chunk = sample_chunk_boundary(ctx);

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        let mut decoder = RtmpChunkDecoder::default();
        let (size, decoded) = decode_one_chunk(&mut decoder, &buf);

        let decoded = decoded.expect("デコード結果のチャンクが存在しない");
        assert_eq!(decoded, chunk, "ラウンドトリップしたチャンクが一致しない");
        assert_eq!(size, buf.len(), "デコーダがすべてのバイトを消費していない");
        Ok(())
    })?;
    Ok(())
}

/// 任意の値についてチャンクストリーム ID の有効範囲が守られることを検証する
#[test]
fn chunk_stream_id_valid_range() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let id = noprop::sample_u32(ctx);
        let parsed = RtmpChunkStreamId::new(id);
        if (2..=65599).contains(&id) {
            let parsed = parsed.expect("有効範囲の値はチャンクストリーム ID として受理される");
            assert_eq!(parsed.get(), id, "チャンクストリーム ID の値が一致しない");
        } else {
            assert!(
                parsed.is_none(),
                "有効範囲外の値 {id} がチャンクストリーム ID として受理された"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// チャンクストリーム ID のエンコードサイズが仕様どおりであることを検証する
#[test]
fn chunk_stream_id_encoding_size() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let id = noprop::sample_u32_in(ctx, 2..=65599);
        let chunk_stream_id = RtmpChunkStreamId::new(id).expect("サンプリングした範囲の ID は有効");
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

        // 基本ヘッダーのサイズを検証する。
        // 形式 0 のメッセージヘッダーは 11 バイト固定。
        let expected_basic_header_size = if id < 64 {
            1
        } else if id < 320 {
            2
        } else {
            3
        };
        let expected_total_size = expected_basic_header_size + 11;
        assert_eq!(
            buf.len(),
            expected_total_size,
            "チャンクストリーム ID {id} の基本ヘッダーサイズが期待値と一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// 拡張タイムスタンプのエンコードを検証する
#[test]
fn extended_timestamp_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let timestamp_ms = noprop::sample_u32_in(ctx, 0..=0x0200_0000);
        let timestamp = RtmpTimestamp::from_millis(timestamp_ms);
        let chunk = RtmpChunk {
            chunk_stream_id: RtmpChunkStreamId::new(4).expect("4 は有効なチャンクストリーム ID"),
            message_stream_id: RtmpMessageStreamId::PCM,
            message_type: RtmpMessageType::Ack,
            timestamp,
            payload: vec![],
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk);

        // 拡張タイムスタンプが必要かどうかで期待サイズが変わる
        let uses_extended = timestamp_ms >= 0x00FF_FFFF;
        let expected_size = if uses_extended { 1 + 11 + 4 } else { 1 + 11 };
        assert_eq!(
            buf.len(),
            expected_size,
            "タイムスタンプ {timestamp_ms} のエンコードサイズが期待値と一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// 同じストリームで連続したチャンクの roundtrip を検証する
#[test]
fn consecutive_chunks_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let chunk1 = sample_chunk(ctx);
        let timestamp_delta = noprop::sample_u32_in(ctx, 0..1000);
        let payload = sample_payload(ctx);

        // 同じストリームで連続するチャンクを作成する
        let chunk2 = RtmpChunk {
            chunk_stream_id: chunk1.chunk_stream_id,
            message_stream_id: chunk1.message_stream_id,
            message_type: chunk1.message_type,
            timestamp: chunk1
                .timestamp
                .wrapping_add(RtmpTimestamp::from_millis(timestamp_delta)),
            payload,
        };

        let mut encoder = RtmpChunkEncoder::default();
        let mut buf = Vec::new();
        encoder.encode(&mut buf, &chunk1);
        encoder.encode(&mut buf, &chunk2);

        let mut decoder = RtmpChunkDecoder::default();
        let mut decoded_chunks = Vec::new();
        let mut remaining = buf.as_slice();

        while !remaining.is_empty() {
            let (size, maybe_chunk) = decoder
                .decode(remaining)
                .expect("エンコードしたデータのデコードは失敗しない");
            remaining = &remaining[size..];
            if let Some(chunk) = maybe_chunk {
                decoded_chunks.push(chunk);
            }
        }

        assert_eq!(
            decoded_chunks.len(),
            2,
            "デコードしたチャンク数が一致しない"
        );
        assert_eq!(decoded_chunks[0], chunk1, "1 つ目のチャンクが一致しない");
        assert_eq!(decoded_chunks[1], chunk2, "2 つ目のチャンクが一致しない");
        Ok(())
    })?;
    Ok(())
}

/// チャンクサイズの境界値が正しく扱われることを検証する
#[test]
fn chunk_size_valid_range() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let size = noprop::sample_usize_in(ctx, 0..=65537);
        let parsed = RtmpChunkSize::new(size);
        if (1..=65536).contains(&size) {
            let parsed = parsed.expect("有効範囲の値はチャンクサイズとして受理される");
            assert_eq!(parsed.get(), size, "チャンクサイズの値が一致しない");
        } else {
            assert!(
                parsed.is_none(),
                "有効範囲外の値 {size} がチャンクサイズとして受理された"
            );
        }
        Ok(())
    })?;
    Ok(())
}
