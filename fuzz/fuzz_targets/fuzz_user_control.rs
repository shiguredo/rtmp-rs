//! User Control Event Fuzzing
//!
//! RTMP 仕様 Section 6.2: User Control Messages
//!
//! User Control メッセージは message type 4 で送信される。
//! Event type (2 bytes) + Event data (variable) で構成される。
//!
//! Event types:
//! - 0: StreamBegin
//! - 1: StreamEof
//! - 2: StreamDry
//! - 3: SetBufferLength
//! - 4: StreamIsRecorded
//! - 6: PingRequest
//! - 7: PingResponse
//! - 31: BufferEmpty
//! - 32: BufferReady

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::RtmpUserControlEvent;

fuzz_target!(|data: &[u8]| {
    // User Control Event のデコードを試みる
    if let Ok(event) = RtmpUserControlEvent::decode(data) {
        // デコードに成功したら、エンコードも試みる
        let mut encoded = Vec::new();
        event.encode(&mut encoded);
    }
});
