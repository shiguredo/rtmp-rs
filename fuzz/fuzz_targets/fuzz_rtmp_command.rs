//! RTMP Command Fuzzing
//!
//! RTMP コマンドのパースをファジングする。

#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{AmfValue, AmfVersion, RtmpCommand, TransactionId};

fuzz_target!(|data: &[u8]| {
    // まず AMF0 値としてデコードを試みる
    if let Ok((_size, object)) = AmfValue::decode(data, AmfVersion::Amf0) {
        // コマンド名として一般的なものを試す
        let command_names = ["connect", "createStream", "publish", "play", "deleteStream", "_result", "onStatus"];
        
        for name in command_names {
            let transaction_id = TransactionId::from_f64(1.0);
            let _ = RtmpCommand::from_message(name, transaction_id, object.clone(), vec![]);
        }
    }
});
