#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_rtmp::tests::{AmfValue, AmfVersion};

fuzz_target!(|data: &[u8]| {
    // AMF0 値のデコードを試みる
    if let Ok((_size, value)) = AmfValue::decode(data, AmfVersion::Amf0) {
        // デコードに成功したら、エンコードも試みる
        let mut encoded = Vec::new();
        value.encode(&mut encoded);
    }
});
