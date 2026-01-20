//! RTMP Server Connection Fuzzing
//!
//! RTMP サーバー接続の受信データ処理をファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::RtmpServerConnection;

fuzz_target!(|data: &[u8]| {
    let mut server = RtmpServerConnection::new();
    let _ = server.feed_recv_buf(data);
});
