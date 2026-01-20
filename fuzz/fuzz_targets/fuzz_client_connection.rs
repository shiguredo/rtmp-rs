//! RTMP Client Connection Fuzzing
//!
//! RTMP クライアント接続の受信データ処理をファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{RtmpPlayClientConnection, RtmpPublishClientConnection};

fuzz_target!(|data: &[u8]| {
    // Publish クライアントのファジング
    {
        let mut client = RtmpPublishClientConnection::new("rtmp://localhost/app", "stream");
        let _ = client.feed_recv_buf(data);
    }
    
    // Play クライアントのファジング
    {
        let mut client = RtmpPlayClientConnection::new("rtmp://localhost/app", "stream");
        let _ = client.feed_recv_buf(data);
    }
});
