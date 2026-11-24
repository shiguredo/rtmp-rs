//! 無視対象の RTMP メッセージを複数の公開受信 API で検証するためのテスト補助

use noprop::TestCaseContext;
use shiguredo_rtmp::flv::encode_audio_frame;
use shiguredo_rtmp::media::{AudioFormat, AudioFrame, AudioSampleRate};
use shiguredo_rtmp::rtmp_chunk::{
    MessageHeaderFormat, RtmpChunk, RtmpChunkSize, RtmpChunkStreamId,
};
use shiguredo_rtmp::rtmp_chunk_encoder::RtmpChunkEncoder;
use shiguredo_rtmp::rtmp_connection::{RtmpConnectionEvent, RtmpConnectionState};
use shiguredo_rtmp::rtmp_message::{
    RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId, RtmpMessageType,
};
use shiguredo_rtmp::rtmp_message_encoder::RtmpMessageEncoder;
use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;

/// 無視対象 2 件を含む入力列と、公開受信 API から取り出せる結果
pub struct IgnoredMessageSample {
    /// 完成したチャンクを 1 個と、次のチャンクの一部を含む最初の入力
    pub incomplete: Vec<u8>,
    /// 別チャンクストリームの音声、連続する無視対象 2 件、後続音声が順に完成する残りの入力
    pub completion: Vec<u8>,
    audio: [(RtmpMessageStreamId, AudioFrame); 2],
    ignored: [(&'static str, RtmpMessageHeader); 2],
}

impl IgnoredMessageSample {
    /// 無視対象を除き、低レベル受信 API から取り出せる音声メッセージを完成順に返す
    pub fn into_audio_messages(self) -> [RtmpMessage; 2] {
        self.audio
            .map(|(stream_id, frame)| audio_message(stream_id, frame))
    }

    /// 接続の公開 API で完成前の無通知、完成順のイベント列、状態の維持を検証する
    pub fn assert_events(
        self,
        expected_state: RtmpConnectionState,
        mut receive: impl FnMut(&[u8]) -> (RtmpConnectionState, Vec<RtmpConnectionEvent>),
    ) {
        let (state, events) = receive(&self.incomplete);
        assert_eq!(state, expected_state, "未完成の受信で状態は変わらないはず");
        assert!(events.is_empty(), "完成前のイベントはないはず: {events:?}");

        // 通知を入力ごとに 1 件へまとめたり、通常メッセージの前後にまとめたりしてはいけない。
        let (state, events) = receive(&self.completion);
        let [(_, earlier), (_, following)] = self.audio;
        let [first, second] =
            self.ignored
                .map(|(name, header)| RtmpConnectionEvent::MessageIgnored {
                    name: name.to_owned(),
                    detail: format!(
                        "message stream ID: {}, timestamp: {} ms",
                        header.stream_id.get(),
                        header.timestamp.as_millis(),
                    ),
                });
        assert_eq!(
            events,
            [
                RtmpConnectionEvent::AudioReceived(earlier),
                first,
                second,
                RtmpConnectionEvent::AudioReceived(following),
            ],
            "同じ入力で完成した 4 件は、ボディを含まない通知 2 件を挟んで完成順に返るはず"
        );
        assert_eq!(
            state, expected_state,
            "無視対象の処理で状態は変わらないはず"
        );
    }
}

/// 各ケースで無視対象の全 3 型を生成し、長さ・タイムスタンプ・分割位置を変える
pub fn sample_ignored_messages(ctx: &mut TestCaseContext) -> [IgnoredMessageSample; 3] {
    // Adobe RTMP 仕様（2012 年 12 月版）7.1.3 節と 7.1.6 節の ID との対応を固定する。
    // 仕様で型の割り当てが変更された場合は対応表と合わせて見直す。
    let types = [
        (RtmpMessageType::SharedObjectAmf3, 16, "SharedObjectAmf3"),
        (RtmpMessageType::SharedObjectAmf0, 19, "SharedObjectAmf0"),
        (RtmpMessageType::Aggregate, 22, "Aggregate"),
    ];
    std::array::from_fn(|i| sample_ignored_message(ctx, types[i], types[(i + 1) % types.len()]))
}

fn sample_ignored_message(
    ctx: &mut TestCaseContext,
    (message_type, type_id, name): (RtmpMessageType, u8, &'static str),
    (next_type, _, next_name): (RtmpMessageType, u8, &'static str),
) -> IgnoredMessageSample {
    let chunk_size = RtmpChunkSize::default().get();
    let payload_len = noprop::sample_usize_in(ctx, (2 * chunk_size + 1)..=(3 * chunk_size));
    let chunk_stream_id = RtmpChunkStreamId::new(noprop::sample_u32_in(ctx, 4..=32))
        .expect("4 から 32 は有効なチャンクストリーム ID");
    let timestamp_ms = if noprop::sample_bool(ctx) {
        noprop::sample_u32_in(ctx, 1..0x00FF_FFFF)
    } else {
        noprop::sample_u32_in(ctx, 0x00FF_FFFF..=u32::MAX - 1001)
    };
    let timestamp = RtmpTimestamp::from_millis(timestamp_ms);
    let stream_id = RtmpMessageStreamId::new(noprop::sample_u32_in(ctx, 1..u32::MAX));
    let mut chunk_encoder = RtmpChunkEncoder::default();
    let mut ignored = Vec::new();
    chunk_encoder.encode(
        &mut ignored,
        &RtmpChunk {
            chunk_stream_id,
            message_stream_id: stream_id,
            message_type,
            timestamp,
            // チャンクとして正しい長さにそろえ、無視するボディの内容には前提を置かない。
            payload: noprop::sample_bytes_vec(ctx, payload_len),
        },
    );

    // Adobe RTMP 仕様（2012 年 12 月版）5.3.1 節では、ここで使う CSID の基本ヘッダーは
    // 1 バイト、メッセージヘッダーは形式 0 が 11 バイトで形式 3 が 0 バイトである。
    // 拡張タイムスタンプがある場合は各チャンクに 4 バイトを足す。ヘッダー形式が変わる場合は見直す。
    assert_eq!(
        ignored[7], type_id,
        "無視対象 {name} のワイヤ上の型 ID が一致するはず"
    );
    let extended = if timestamp_ms >= 0x00FF_FFFF { 4 } else { 0 };
    let first_chunk_end = 1 + 11 + extended + chunk_size;
    let second_chunk_end = first_chunk_end + 1 + extended + chunk_size;
    let split = noprop::sample_usize_in(ctx, (first_chunk_end + 1)..second_chunk_end);
    let last_chunk = ignored.split_off(second_chunk_end);
    let mut completion = ignored.split_off(split);

    // 別 CSID の音声を最後のチャンクの前に入れ、メッセージの開始順と完成順が異なるようにする。
    // 時刻とストリーム ID を無視対象と変え、時刻による並べ替えや履歴の混入を検出する。
    let earlier = sample_audio_frame(ctx, RtmpTimestamp::from_millis(timestamp_ms + 2));
    let other_stream_id = RtmpMessageStreamId::new(stream_id.get() + 1);
    let other_csid = RtmpChunkStreamId::new(63).expect("63 は有効なチャンクストリーム ID");
    RtmpMessageEncoder::default()
        .encode(
            &mut completion,
            other_csid,
            audio_message(other_stream_id, earlier.clone()),
        )
        .expect("生成した音声メッセージはエンコードできるはず");
    completion.extend_from_slice(&last_chunk);

    // 異なる無視対象を同じチャンクストリームに続け、両方が同じ入力で完成するようにする。
    let next_timestamp = RtmpTimestamp::from_millis(timestamp_ms + 1);
    let next_len = noprop::sample_usize_in(ctx, 1..=chunk_size);
    chunk_encoder.encode(
        &mut completion,
        &RtmpChunk {
            chunk_stream_id,
            message_stream_id: stream_id,
            message_type: next_type,
            timestamp: next_timestamp,
            payload: noprop::sample_bytes_vec(ctx, next_len),
        },
    );

    // 最後の音声は同じ CSID とメッセージストリーム ID を使い、型を変えて形式 1 を選ばせる。
    // 形式 1 が時刻とストリーム ID を復元するには、無視対象のヘッダー履歴が必要になる。
    let delta_ms = noprop::sample_u32_in(ctx, 1..=1000);
    let following =
        sample_audio_frame(ctx, RtmpTimestamp::from_millis(timestamp_ms + 1 + delta_ms));
    let mut payload = Vec::new();
    encode_audio_frame(&mut payload, &following);
    let following_start = completion.len();
    chunk_encoder.encode(
        &mut completion,
        &RtmpChunk {
            chunk_stream_id,
            message_stream_id: stream_id,
            message_type: RtmpMessageType::Audio,
            timestamp: following.timestamp,
            payload,
        },
    );
    assert_eq!(
        completion[following_start] >> 6,
        MessageHeaderFormat::F1 as u8,
        "後続の音声は形式 1 の圧縮ヘッダーを使うはず"
    );

    IgnoredMessageSample {
        incomplete: ignored,
        completion,
        audio: [(other_stream_id, earlier), (stream_id, following)],
        ignored: [(name, timestamp), (next_name, next_timestamp)].map(|(name, timestamp)| {
            (
                name,
                RtmpMessageHeader {
                    stream_id,
                    timestamp,
                },
            )
        }),
    }
}

fn sample_audio_frame(ctx: &mut TestCaseContext, timestamp: RtmpTimestamp) -> AudioFrame {
    let len = noprop::sample_usize_in(ctx, 1..=32);
    AudioFrame {
        timestamp,
        format: AudioFormat::Mp3,
        sample_rate: AudioSampleRate::Khz44,
        is_8bit_sample: false,
        is_stereo: true,
        is_aac_sequence_header: false,
        data: noprop::sample_bytes_vec(ctx, len),
    }
}

fn audio_message(stream_id: RtmpMessageStreamId, frame: AudioFrame) -> RtmpMessage {
    RtmpMessage::Audio {
        header: RtmpMessageHeader {
            stream_id,
            timestamp: frame.timestamp,
        },
        frame,
    }
}
