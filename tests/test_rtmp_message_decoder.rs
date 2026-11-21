//! RTMP Message Decoder の分岐テスト
//!
//! プロパティとして表現しにくい意図的なエラーパスと、特殊な入力の扱いを検証する。

use shiguredo_rtmp::amf::{AmfValue, AmfVersion};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::error::ErrorKind;
use shiguredo_rtmp::rtmp_chunk::{RtmpChunk, RtmpChunkStreamId};
use shiguredo_rtmp::rtmp_chunk_encoder::RtmpChunkEncoder;
use shiguredo_rtmp::rtmp_message::{RtmpMessage, RtmpMessageStreamId, RtmpMessageType};
use shiguredo_rtmp::rtmp_message_decoder::RtmpMessageDecoder;
use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;

/// 指定した MessageType と Payload からチャンクを組み立てる
fn encode_chunk(message_type: RtmpMessageType, payload: Vec<u8>) -> Vec<u8> {
    let chunk = RtmpChunk {
        chunk_stream_id: RtmpChunkStreamId::new(3).expect("3 は有効なチャンクストリーム ID"),
        message_stream_id: RtmpMessageStreamId::PCM,
        message_type,
        timestamp: RtmpTimestamp::from_millis(0),
        payload,
    };
    let mut encoder = RtmpChunkEncoder::default();
    let mut buf = Vec::new();
    encoder.encode(&mut buf, &chunk);
    buf
}

/// AMF3 Command の 0 先頭は AMF0 として扱われることを検証
#[test]
fn command_amf3_zero_prefix_treated_as_amf0() {
    let mut payload = Vec::new();
    payload.push(0);
    AmfValue::from((AmfVersion::Amf0, "connect")).encode(&mut payload);
    AmfValue::from((AmfVersion::Amf0, 1.0_f64)).encode(&mut payload);
    AmfValue::Amf0(Amf0Value::Null).encode(&mut payload);

    let buf = encode_chunk(RtmpMessageType::CommandAmf3, payload);
    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&buf);
    let message = decoder
        .decode()
        .expect("エンコード済みメッセージのデコードは失敗しない")
        .expect("メッセージが 1 つ存在する");

    match message {
        RtmpMessage::Command {
            amf_version,
            name,
            transaction_id,
            args,
            ..
        } => {
            assert_eq!(
                amf_version,
                AmfVersion::Amf0,
                "AMF のバージョンが一致しない"
            );
            assert_eq!(name, "connect", "コマンド名が一致しない");
            assert_eq!(transaction_id.get(), 1, "トランザクション ID が一致しない");
            assert!(args.is_empty(), "追加の引数が空でない");
        }
        _ => panic!("想定外のメッセージ種別である"),
    }
}

/// SetPeerBandwidth の limit_type 不正値がエラーになることを検証
#[test]
fn set_peer_bandwidth_invalid_limit_type() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&100u32.to_be_bytes());
    payload.push(9);

    let buf = encode_chunk(RtmpMessageType::SetPeerBandwidth, payload);
    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&buf);
    let err = decoder
        .decode()
        .expect_err("不正な limit type のデコードは失敗する");
    assert_eq!(err.kind, ErrorKind::InvalidData, "エラー種別が一致しない");
}

/// UserControl の未知 event_type がエラーになることを検証
#[test]
fn user_control_unknown_event_type() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&999u16.to_be_bytes());

    let buf = encode_chunk(RtmpMessageType::UserControl, payload);
    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&buf);
    let err = decoder
        .decode()
        .expect_err("未知のユーザーコントロールイベントのデコードは失敗する");
    assert_eq!(err.kind, ErrorKind::InvalidData, "エラー種別が一致しない");
}

/// 不足バッファでは None を返すことを検証
#[test]
fn insufficient_buffer_returns_none() {
    let mut decoder = RtmpMessageDecoder::default();
    decoder.feed_buf(&[0]);
    let result = decoder
        .decode()
        .expect("途中までの入力のデコードは失敗しない");
    assert!(
        result.is_none(),
        "メッセージが揃っていない場合は None になる"
    );
}
