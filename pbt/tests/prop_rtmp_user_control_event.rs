//! User Control Event の Property-Based Testing

use noprop::TestCaseContext;
use shiguredo_rtmp::error::ErrorKind;
use shiguredo_rtmp::rtmp_message::RtmpMessageStreamId;
use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;
use shiguredo_rtmp::rtmp_user_control_event::RtmpUserControlEvent;

/// このファイルの PBT ケース数 (旧テストの 300 ケースを維持する)
const CASES: usize = 300;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// メッセージストリーム ID を生成する
fn sample_stream_id(ctx: &mut TestCaseContext) -> RtmpMessageStreamId {
    RtmpMessageStreamId::new(noprop::sample_u32(ctx))
}

/// 各イベントに必要なフィールドをサンプリングし、対応する全種類を生成する
fn sample_events(ctx: &mut TestCaseContext) -> [RtmpUserControlEvent; 9] {
    [
        RtmpUserControlEvent::StreamBegin {
            stream_id: sample_stream_id(ctx),
        },
        RtmpUserControlEvent::StreamEof {
            stream_id: sample_stream_id(ctx),
        },
        RtmpUserControlEvent::StreamDry {
            stream_id: sample_stream_id(ctx),
        },
        RtmpUserControlEvent::SetBufferLength {
            stream_id: sample_stream_id(ctx),
            length: noprop::sample_u32(ctx),
        },
        RtmpUserControlEvent::StreamIsRecorded {
            stream_id: sample_stream_id(ctx),
        },
        RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        RtmpUserControlEvent::BufferEmpty {
            stream_id: sample_stream_id(ctx),
        },
        RtmpUserControlEvent::BufferReady {
            stream_id: sample_stream_id(ctx),
        },
    ]
}

/// 全既知型で必要な長さに満たない入力と、余分なバイト付きの入力を区別する
#[test]
fn user_control_payload_boundaries() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let suffix_len = noprop::sample_usize_in(ctx, 1..=16);
        let suffix = noprop::sample_bytes_vec(ctx, suffix_len);
        for event in sample_events(ctx) {
            let mut buf = Vec::new();
            event.encode(&mut buf);
            let name = event.name();

            // Adobe RTMP 仕様 6.2 節と 7.1.7 節のイベントタイプは 2 バイト、
            // SetBufferLength のデータは 8 バイト、ほかの標準イベントは 4 バイト。
            // 既存の BufferEmpty と BufferReady も 4 バイトのストリーム ID を持つ。
            let expected_len = if matches!(event, RtmpUserControlEvent::SetBufferLength { .. }) {
                10
            } else {
                6
            };
            assert_eq!(buf.len(), expected_len, "{name} のペイロード長が異なる");

            // 空、イベントタイプの途中、各フィールドの途中を含めてすべて確認する。
            for len in 0..expected_len {
                let error = RtmpUserControlEvent::decode(&buf[..len])
                    .expect_err("必要なデータに満たない完成済みペイロードはデコードできないはず");
                assert_eq!(
                    error.kind,
                    ErrorKind::InvalidData,
                    "{name} の {len} バイトのペイロードは入力データ不正になるはず"
                );
            }

            let decoded = RtmpUserControlEvent::decode(&buf).expect("必要な長さの入力は有効なはず");
            assert_eq!(decoded, event, "{name} のデコード結果が元と一致しない");
            buf.extend_from_slice(&suffix);
            let decoded = RtmpUserControlEvent::decode(&buf)
                .expect("ペイロード末尾の余分なバイトはデコードに影響しないはず");
            assert_eq!(decoded, event, "末尾の追加で {name} の内容が変わった");
        }
        Ok(())
    })?;
    Ok(())
}

/// データの長さに関係なく未知のイベントタイプは無効と判定する
#[test]
fn unknown_user_control_event_type_is_invalid_data() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let len = noprop::sample_usize_in(ctx, 0..=16);
        let data = noprop::sample_bytes_vec(ctx, len);
        // 既知型の間にある値と、最後の既知型より大きい値をそれぞれ確認する。
        for event_type in [
            5,
            noprop::sample_u16_in(ctx, 8..=30),
            noprop::sample_u16_in(ctx, 33..=u16::MAX),
        ] {
            let mut buf = event_type.to_be_bytes().to_vec();
            buf.extend_from_slice(&data);
            assert_eq!(
                RtmpUserControlEvent::decode(&buf)
                    .expect_err("未知のイベントタイプはデコードできないはず")
                    .kind,
                ErrorKind::InvalidData,
                "イベントタイプ {event_type} は入力データ不正になるはず"
            );
        }
        Ok(())
    })?;
    Ok(())
}
