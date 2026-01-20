#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{AmfValue, AmfVersion};

fuzz_target!(|data: &[u8]| {
    // AMF3 値のデコードを試みる
    if let Ok((_size, value)) = AmfValue::decode(data, AmfVersion::Amf3) {
        // デコードに成功したら、エンコードも試みる
        let mut encoded = Vec::new();
        value.encode(&mut encoded);
    }
});
