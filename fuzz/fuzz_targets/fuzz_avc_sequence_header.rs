//! AVC Sequence Header Fuzzing
//!
//! AvcSequenceHeader のバイナリパース処理をファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::AvcSequenceHeader;

fuzz_target!(|data: &[u8]| {
    if let Ok(header) = AvcSequenceHeader::from_bytes(data) {
        header.to_bytes().unwrap();
    }
});
