//! User Control Event の Property-Based Testing

use noprop::TestCaseContext;
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

/// ユーザーコントロールイベントを生成する
///
/// 9 種類のイベントを等確率で選び、イベントごとに必要なフィールドを
/// その場でサンプリングする。イベントの種類とフィールドの対応は
/// [`RtmpUserControlEvent`] の variant 定義のとおりである。
fn sample_event(ctx: &mut TestCaseContext) -> RtmpUserControlEvent {
    // 9 分岐を等確率で選ぶ。分岐をまたいでサンプリングする値の型が
    // 異なっても、選んだ分岐のフィールドだけをサンプリングする。
    match noprop::sample_weighted_index(ctx, &[1; 9]) {
        0 => RtmpUserControlEvent::StreamBegin {
            stream_id: sample_stream_id(ctx),
        },
        1 => RtmpUserControlEvent::StreamEof {
            stream_id: sample_stream_id(ctx),
        },
        2 => RtmpUserControlEvent::StreamDry {
            stream_id: sample_stream_id(ctx),
        },
        3 => {
            let stream_id = sample_stream_id(ctx);
            let length = noprop::sample_u32(ctx);
            RtmpUserControlEvent::SetBufferLength { stream_id, length }
        }
        4 => RtmpUserControlEvent::StreamIsRecorded {
            stream_id: sample_stream_id(ctx),
        },
        5 => RtmpUserControlEvent::PingRequest {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        6 => RtmpUserControlEvent::PingResponse {
            timestamp: RtmpTimestamp::from_millis(noprop::sample_u32(ctx)),
        },
        7 => RtmpUserControlEvent::BufferEmpty {
            stream_id: sample_stream_id(ctx),
        },
        _ => RtmpUserControlEvent::BufferReady {
            stream_id: sample_stream_id(ctx),
        },
    }
}

/// UserControlEvent の encode / decode が可逆であることを検証する
#[test]
fn user_control_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let event = sample_event(ctx);

        // イベントをバイト列へエンコードし、同じバイト列からデコードする
        let mut buf = Vec::new();
        event.encode(&mut buf);
        let decoded =
            RtmpUserControlEvent::decode(&buf).expect("エンコードしたイベントのデコードに失敗した");

        // デコード結果は元のイベントと完全に一致するはずである
        assert_eq!(
            decoded, event,
            "エンコードとデコードでイベントの内容が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}
