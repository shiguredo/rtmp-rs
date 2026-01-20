//! RTMP Handshake Fuzzing
//!
//! RTMP 仕様 Section 5.2: Handshake
//!
//! ハンドシェイクは C0/S0 (1 byte) + C1/S1 (1536 bytes) + C2/S2 (1536 bytes) で構成される。
//! このファザーは、不正なハンドシェイクデータに対するサーバーとクライアントの耐性をテストする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{RtmpClientHandshake, RtmpServerHandshake};

fuzz_target!(|data: &[u8]| {
    // サーバー側ハンドシェイクのファジング
    {
        let mut server = RtmpServerHandshake::new();
        let _ = server.feed_recv_buf(data);
    }

    // クライアント側ハンドシェイクのファジング
    {
        let mut client = RtmpClientHandshake::new();
        let _ = client.feed_recv_buf(data);
    }
});
