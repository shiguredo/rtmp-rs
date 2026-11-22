//! FLV 音声・映像フレームのファジング
//!
//! FLV 形式の音声・映像フレームのデコードをファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::flv::{
    decode_audio_frame, decode_video_frame, encode_audio_frame, encode_video_frame,
};
use shiguredo_rtmp::rtmp_timestamp::RtmpTimestamp;

fuzz_target!(|data: &[u8]| {
    let timestamp = RtmpTimestamp::ZERO;

    // オーディオフレームのデコードを試みる
    if let Ok(frame) = decode_audio_frame(data, timestamp) {
        // デコード成功した場合、エンコードも試みる
        let mut encoded = Vec::new();
        encode_audio_frame(&mut encoded, &frame);
    }

    // ビデオフレームのデコードを試みる
    if let Ok(frame) = decode_video_frame(data, timestamp) {
        // デコードに成功したフレームのコーデックと AVC パケットタイプは矛盾しない。
        let mut encoded = Vec::new();
        encode_video_frame(&mut encoded, &frame)
            .expect("デコードした映像フレームのエンコードは失敗しない");
    }
});
