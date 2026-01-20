#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::RtmpMessageDecoder;

fuzz_target!(|data: &[u8]| {
    let mut decoder = RtmpMessageDecoder::default();

    // メッセージのデコードを試みる
    decoder.feed_buf(data);
    loop {
        match decoder.decode() {
            Ok(Some(_message)) => {}
            Ok(None) => break,
            Err(_) => break,
        }
    }
});
