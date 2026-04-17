//! RTMP Client Connection Fuzzing
//!
//! RTMP クライアント接続の受信データ処理をファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::RtmpUrl;
use shiguredo_rtmp::tests::{RtmpPlayClientConnection, RtmpPublishClientConnection};

fuzz_target!(|data: &[u8]| {
    // 固定の接続先 URL (ファジングの対象はあくまで受信バッファなので URL は常に同じ定数でよい)
    let url = RtmpUrl::parse("rtmp://localhost/app/stream").expect("valid RTMP URL");

    // Publish クライアントのファジング
    {
        let mut client = RtmpPublishClientConnection::new(url.clone());
        let _ = client.feed_recv_buf(data);
    }

    // Play クライアントのファジング
    {
        let mut client = RtmpPlayClientConnection::new(url);
        let _ = client.feed_recv_buf(data);
    }
});
