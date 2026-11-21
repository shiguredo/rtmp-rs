//! AMF のゴールデンファイル検証
//!
//! `std::fs` は統合テスト側に置き、`src/` は `core` / `alloc` のみとする。

use shiguredo_rtmp::amf::{AmfValue, AmfVersion};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::error::ErrorKind;

/// 指定したバージョンの AMF テストデータをデコード・再エンコードして検証する
fn verify_amf_testdata(prefix: &str, version: AmfVersion) {
    for entry in
        std::fs::read_dir("tests/testdata").expect("テストデータディレクトリの読み込みに失敗した")
    {
        let entry = entry.expect("テストデータディレクトリのエントリ取得に失敗した");

        // 対象のテストデータかどうかをまず確認する
        let test_file_path = entry.path();
        let Some(test_file_name) = test_file_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !test_file_name.starts_with(prefix) || !test_file_name.ends_with(".bin") {
            continue;
        }

        // AMF のバイナリをデコードしてからエンコードし、
        // その結果が元のバイナリと一致するかを確認する
        // （AMF 自体は枯れたフォーマットでそこまで複雑でもないので、
        //   各値のデコード結果を細かくは確認しない）
        println!("テストデータ: {test_file_name}");

        let original_data =
            std::fs::read(&test_file_path).expect("テストデータの読み込みに失敗した");

        // 異常系のテストデータは期待するエラー種別だけを確認する
        if test_file_name.ends_with("-partial.bin") {
            let err = AmfValue::decode(&original_data, version)
                .expect_err("途中で切れたテストデータのデコードが成功してしまった");
            assert_eq!(
                err.kind,
                ErrorKind::InsufficientBuffer,
                "エラー種別が一致しない"
            );
            continue;
        }
        if test_file_name.contains("-bad-") {
            let err = AmfValue::decode(&original_data, version)
                .expect_err("不正なテストデータのデコードが成功してしまった");
            assert_eq!(err.kind, ErrorKind::InvalidData, "エラー種別が一致しない");
            continue;
        }
        if test_file_name.contains("-unsupported-") {
            let err = AmfValue::decode(&original_data, version)
                .expect_err("未対応のテストデータのデコードが成功してしまった");
            assert_eq!(err.kind, ErrorKind::Unsupported, "エラー種別が一致しない");
            continue;
        }

        let (decoded_len, amf_value) =
            AmfValue::decode(&original_data, version).expect("AMF のデコードに失敗した");
        assert_eq!(
            decoded_len,
            original_data.len(),
            "デコードで消費した長さが一致しない"
        );

        let mut encoded_data = Vec::new();
        amf_value.encode(&mut encoded_data);

        // もう一度デコードして、その値を `amf_value` と比較する
        // （参照型の場合には `encoded_data` と `original_data` が一致しないことがあるため）
        let (re_decoded_len, re_decoded_amf_value) = AmfValue::decode(&encoded_data, version)
            .expect("再エンコードしたデータのデコードに失敗した");
        assert_eq!(
            re_decoded_len,
            encoded_data.len(),
            "再デコードで消費した長さが一致しない"
        );

        if let AmfValue::Amf0(Amf0Value::Number(n0)) = amf_value
            && let AmfValue::Amf0(Amf0Value::Number(n1)) = re_decoded_amf_value
            && n0.is_nan()
            && n1.is_nan()
        {
            // NaN 同士は比較できないため、このテストデータは比較をスキップする
            continue;
        }

        assert_eq!(
            amf_value, re_decoded_amf_value,
            "ラウンドトリップした値が一致しない"
        );
    }
}

#[test]
fn decode_and_encode_amf0_values() {
    verify_amf_testdata("amf0", AmfVersion::Amf0);
}

#[test]
fn decode_and_encode_amf3_values() {
    verify_amf_testdata("amf3", AmfVersion::Amf3);
}
