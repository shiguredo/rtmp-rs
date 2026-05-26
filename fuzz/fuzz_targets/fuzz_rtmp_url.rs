//! RTMP URL Fuzzing
//!
//! RtmpUrl のパース処理をファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::RtmpUrl;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = core::str::from_utf8(data) else {
        return;
    };

    if let Ok(url) = RtmpUrl::parse(s) {
        let _ = url.to_string();
    }

    // fuzzer の入力から URL とストリーム名の 2 引数を生成するために先頭 1 バイトを分割位置に流用する
    if data.len() >= 2 {
        let split_byte = data[0] as usize;
        let remaining = &data[1..];
        let split_pos = split_byte % remaining.len();
        let (url_part, stream_part) = remaining.split_at(split_pos);

        if let (Ok(url_str), Ok(stream_str)) =
            (core::str::from_utf8(url_part), core::str::from_utf8(stream_part))
        {
            if let Ok(url) = RtmpUrl::parse_with_stream_name(url_str, stream_str) {
                let _ = url.to_string();
            }
        }
    }
});
