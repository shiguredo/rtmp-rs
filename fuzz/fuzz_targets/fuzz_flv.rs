//! FLV Audio/Video Frame Fuzzing
//!
//! FLV 形式のオーディオ/ビデオフレームのデコードをファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{decode_audio_frame, decode_video_frame, RtmpTimestamp};

fuzz_target!(|data: &[u8]| {
    let timestamp = RtmpTimestamp::ZERO;
    
    // オーディオフレームのデコードを試みる
    if let Ok(frame) = decode_audio_frame(data, timestamp) {
        // デコード成功した場合、エンコードも試みる
        let mut encoded = Vec::new();
        shiguredo_rtmp::tests::encode_audio_frame(&mut encoded, &frame);
    }
    
    // ビデオフレームのデコードを試みる
    if let Ok(frame) = decode_video_frame(data, timestamp) {
        // デコード成功した場合、エンコードも試みる
        let mut encoded = Vec::new();
        shiguredo_rtmp::tests::encode_video_frame(&mut encoded, &frame);
    }
});
