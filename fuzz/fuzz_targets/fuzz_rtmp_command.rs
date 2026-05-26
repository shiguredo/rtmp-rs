//! RTMP Command Fuzzing
//!
//! RTMP コマンドのパースをファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{AmfValue, AmfVersion, RtmpCommand, TransactionId};

fuzz_target!(|data: &[u8]| {
    // 入力バイト列全体を AMF0 値として連続デコードし、最初の値を object、2 番目以降を args とする
    let mut offset = 0;
    let mut values = Vec::new();
    while offset < data.len() {
        match AmfValue::decode(&data[offset..], AmfVersion::Amf0) {
            Ok((size, value)) => {
                values.push(value);
                offset += size;
            }
            Err(_) => break,
        }
    }

    if values.is_empty() {
        return;
    }

    let object = values.remove(0);
    let args = values;

    let command_names = [
        "connect",
        "createStream",
        "publish",
        "play",
        "deleteStream",
        "getStreamLength",
        "_result",
        "onStatus",
    ];

    for name in command_names {
        let transaction_id = TransactionId::from_f64(1.0);
        let _ = RtmpCommand::from_message(name, transaction_id, object.clone(), args.clone());
    }
});
