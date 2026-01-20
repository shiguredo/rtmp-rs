#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::RtmpChunkDecoder;

fuzz_target!(|data: &[u8]| {
    let mut decoder = RtmpChunkDecoder::default();
    let mut remaining = data;

    // 複数のチャンクをデコードし続ける
    while !remaining.is_empty() {
        match decoder.decode(remaining) {
            Ok((size, _maybe_chunk)) => {
                if size == 0 {
                    break;
                }
                remaining = &remaining[size..];
            }
            Err(_) => break,
        }
    }
});
