//! AMF のゴールデンファイル検証（`std::fs` は統合テスト側に置き、`src/` は `core` / `alloc` のみとする）

use shiguredo_rtmp::{Amf0Value, AmfValue, AmfVersion, ErrorKind};

#[test]
fn decode_and_encode_amf0_values() {
    for entry in std::fs::read_dir("tests/testdata").expect("read_dir() error") {
        let entry = entry.expect("read_dir() error");

        // 対象のテストデータかどうかをまずチェックする
        let test_file_path = entry.path();
        let Some(test_file_name) = test_file_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !test_file_name.starts_with("amf0") || !test_file_name.ends_with(".bin") {
            continue;
        }

        // AMF のバイナリをデコードしてからエンコードして、
        // その結果バイナリが一致するかどうかを確認する
        // （AMF自体は枯れたフォーマットでそこまで複雑でもないので、
        //   各値のデコード結果を細かくは確認しない）
        println!("TEST_DATA: {test_file_name}");

        let original_data = std::fs::read(&test_file_path).expect("read() error");
        let version = AmfVersion::Amf0;

        // 異常系のテストデータは特別扱いする
        if test_file_name.ends_with("-partial.bin") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::InsufficientBuffer);
            continue;
        }
        if test_file_name.contains("-bad-") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::InvalidData);
            continue;
        }
        if test_file_name.contains("-unsupported-") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::Unsupported);
            continue;
        }

        let (decoded_len, amf_value) =
            AmfValue::decode(&original_data, version).expect("AmfValue::decode() error");
        assert_eq!(decoded_len, original_data.len());

        let mut encoded_data = Vec::new();
        amf_value.encode(&mut encoded_data);

        // もう一回デコードして、その値を `amf_value` と比較する
        //（参照型の場合には `encoded_data` と `original_data` が一致しないことがあるため）
        let (re_decoded_len, re_decoded_amf_value) = AmfValue::decode(&encoded_data, version)
            .expect("AmfValue::decode() error on re-decode");
        assert_eq!(re_decoded_len, encoded_data.len());

        if let AmfValue::Amf0(Amf0Value::Number(n0)) = amf_value
            && let AmfValue::Amf0(Amf0Value::Number(n1)) = re_decoded_amf_value
            && n0.is_nan()
            && n1.is_nan()
        {
            // NaN 同士は比較できない
            continue;
        }

        assert_eq!(amf_value, re_decoded_amf_value);
    }
}

#[test]
fn decode_and_encode_amf3_values() {
    for entry in std::fs::read_dir("tests/testdata").expect("read_dir() error") {
        let entry = entry.expect("read_dir() error");

        // 対象のテストデータかどうかをまずチェックする
        let test_file_path = entry.path();
        let Some(test_file_name) = test_file_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !test_file_name.starts_with("amf3") || !test_file_name.ends_with(".bin") {
            continue;
        }

        // AMF のバイナリをデコードしてからエンコードして、
        // その結果バイナリが一致するかどうかを確認する
        // （AMF自体は枯れたフォーマットでそこまで複雑でもないので、
        //   各値のデコード結果を細かくは確認しない）
        println!("TEST_DATA: {test_file_name}");

        let original_data = std::fs::read(&test_file_path).expect("read() error");
        let version = AmfVersion::Amf3;

        // 異常系のテストデータは特別扱いする
        if test_file_name.ends_with("-partial.bin") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::InsufficientBuffer);
            continue;
        }
        if test_file_name.contains("-bad-") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::InvalidData);
            continue;
        }
        if test_file_name.contains("-unsupported-") {
            let err = AmfValue::decode(&original_data, version).expect_err("AmfValue::decode() success");
            assert_eq!(err.kind, ErrorKind::Unsupported);
            continue;
        }

        let (decoded_len, amf_value) =
            AmfValue::decode(&original_data, version).expect("AmfValue::decode() error");
        assert_eq!(decoded_len, original_data.len());

        let mut encoded_data = Vec::new();
        amf_value.encode(&mut encoded_data);

        // もう一回デコードして、その値を `amf_value` と比較する
        //（参照型の場合には `encoded_data` と `original_data` が一致しないことがあるため）
        let (re_decoded_len, re_decoded_amf_value) = AmfValue::decode(&encoded_data, version)
            .expect("AmfValue::decode() error on re-decode");
        assert_eq!(re_decoded_len, encoded_data.len());
        assert_eq!(amf_value, re_decoded_amf_value);
    }
}
