//! AMF0/AMF3 の Property-Based Testing
//!
//! AMF エンコード/デコードのラウンドトリップテストを行う。

use std::time::Duration;

use noprop::TestCaseContext;
use shiguredo_rtmp::amf::{AmfValue, AmfVersion, Pair};
use shiguredo_rtmp::amf0::Amf0Value;
use shiguredo_rtmp::amf3::Amf3Value;

/// AMF0/AMF3 本体のプロパティのケース数
const CASES: usize = 1000;

/// AmfValue (AMF0/AMF3 の共通ラッパー) のプロパティのケース数
const WRAPPER_CASES: usize = 500;

/// 仕様に基づくエンコーディング検証のケース数
const ENCODING_CASES: usize = 100;

/// 乱数シードを取得する環境変数名
const SEED_ENV: &str = "RTMP_RS_PBT_SEED";

/// 再帰的に生成する AMF 値の最大深度
const MAX_DEPTH: usize = 3;

// =============================================================================
// AMF0 値のサンプラー
// =============================================================================

/// AMF0 の String をサンプリングする (長さは 0..100 文字)
fn sample_amf0_string(ctx: &mut TestCaseContext) -> String {
    // AMF0 の通常文字列は 0xFFFF (65535) バイトまで表現できるため、
    // 1 文字最大 4 バイトでも 400 バイト弱に収まり、長さ制限に抵触しない
    let len = noprop::sample_usize_in(ctx, 0..100);
    noprop::sample_string(ctx, len)
}

/// AMF0 の Number をサンプリングする (特殊値を含む)
fn sample_amf0_number(ctx: &mut TestCaseContext) -> f64 {
    // 6 分岐を等確率で選択する
    // 通常の有限数値は sample_f64 で生成する (ゼロや非正規化数も含む)
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1]) {
        0 => noprop::sample_f64(ctx),
        1 => 0.0,
        2 => -0.0,
        3 => f64::INFINITY,
        4 => f64::NEG_INFINITY,
        // NaN はラウンドトリップで比較できないが、エンコード/デコード自体は可能
        _ => f64::NAN,
    }
}

/// AMF0 の Date 用の Duration をサンプリングする
fn sample_amf0_date(ctx: &mut TestCaseContext) -> Duration {
    // 非負で有限な値のみ (デコーダの制約)
    // Duration::MAX は f64 に変換すると精度が落ちるので、適度な範囲に制限する
    Duration::from_millis(noprop::sample_u64_in(ctx, 0..=253_402_300_799_999))
}

/// AMF0 のリーフ値 (再帰なし) をサンプリングする
fn sample_amf0_leaf(ctx: &mut TestCaseContext) -> Amf0Value {
    // 7 分岐を等確率で選択する
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1, 1]) {
        0 => Amf0Value::Number(sample_amf0_number(ctx)),
        1 => Amf0Value::Boolean(noprop::sample_bool(ctx)),
        2 => Amf0Value::String(sample_amf0_string(ctx)),
        3 => Amf0Value::Null,
        4 => Amf0Value::Undefined,
        5 => Amf0Value::Date {
            unix_time: sample_amf0_date(ctx),
        },
        _ => Amf0Value::XmlDocument(sample_amf0_string(ctx)),
    }
}

/// キーと値の組を 0 個以上 max_len 個未満サンプリングする
///
/// `depth` は各値の子要素の深さを表す。
fn sample_amf0_pairs(
    ctx: &mut TestCaseContext,
    depth: usize,
    max_len: usize,
) -> Vec<Pair<String, Amf0Value>> {
    let len = noprop::sample_usize_in(ctx, 0..max_len);
    (0..len)
        .map(|_| {
            let key = sample_amf0_string(ctx);
            let value = sample_amf0_value(ctx, depth);
            Pair { key, value }
        })
        .collect()
}

/// AMF0 の値を 0 個以上 max_len 個未満サンプリングする
///
/// `depth` は各値の子要素の深さを表す。
fn sample_amf0_values(ctx: &mut TestCaseContext, depth: usize, max_len: usize) -> Vec<Amf0Value> {
    let len = noprop::sample_usize_in(ctx, 0..max_len);
    (0..len).map(|_| sample_amf0_value(ctx, depth)).collect()
}

/// 深さ `depth` の AMF0 値をサンプリングする
///
/// `depth` が 0 なら必ずリーフ値を返す。それ以外は 50% の確率でリーフ値を返し、
/// 残りの 50% を 4 種類の複合値から等確率で選ぶ。子要素の深さは 1 だけ減らすため、
/// 再帰は `depth` で必ず止まり、総ノード数も深さと子要素数の上限で制限される。
fn sample_amf0_value(ctx: &mut TestCaseContext, depth: usize) -> Amf0Value {
    if depth == 0 || noprop::sample_bool(ctx) {
        return sample_amf0_leaf(ctx);
    }
    // 4 分岐を等確率で選択する
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1]) {
        // Object (無名)
        0 => Amf0Value::Object {
            class_name: None,
            entries: sample_amf0_pairs(ctx, depth - 1, 5),
        },
        // Object (名前付き)
        1 => Amf0Value::Object {
            class_name: Some(sample_amf0_string(ctx)),
            entries: sample_amf0_pairs(ctx, depth - 1, 5),
        },
        // EcmaArray (連想配列)
        2 => Amf0Value::EcmaArray {
            entries: sample_amf0_pairs(ctx, depth - 1, 5),
        },
        // StrictArray (密な配列)
        _ => Amf0Value::Array {
            entries: sample_amf0_values(ctx, depth - 1, 5),
        },
    }
}

// =============================================================================
// AMF3 値のサンプラー
// =============================================================================

/// AMF3 の Integer をサンプリングする (i29 範囲)
fn sample_amf3_integer(ctx: &mut TestCaseContext) -> i32 {
    // AMF3 Integer は 29 bit の符号付き整数: -268435456 から 268435455 まで
    noprop::sample_i32_in(ctx, -268_435_456..=268_435_455)
}

/// AMF3 の String をサンプリングする (長さは 0..100 文字)
fn sample_amf3_string(ctx: &mut TestCaseContext) -> String {
    let len = noprop::sample_usize_in(ctx, 0..100);
    noprop::sample_string(ctx, len)
}

/// AMF3 のオブジェクトキーとして使用できる String をサンプリングする
///
/// AMF3 では空文字列が終端マーカーとして使われるため、キーには使えない。
fn sample_amf3_key(ctx: &mut TestCaseContext) -> String {
    let len = noprop::sample_usize_in(ctx, 1..50);
    noprop::sample_string(ctx, len)
}

/// AMF3 の ObjectVector クラス名として使用できる String をサンプリングする
///
/// AMF3 では "*" が「任意の型」を意味するため、クラス名には使えない。
fn sample_amf3_class_name(ctx: &mut TestCaseContext) -> String {
    noprop::sample_with_rejection(ctx, 8, |ctx| {
        let s = sample_amf3_key(ctx);
        (s != "*").then_some(s)
    })
}

/// AMF3 の Double をサンプリングする (特殊値を含む)
fn sample_amf3_double(ctx: &mut TestCaseContext) -> f64 {
    // 6 分岐を等確率で選択する
    // 通常の有限数値は sample_f64 で生成する (ゼロや非正規化数も含む)
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1, 1]) {
        0 => noprop::sample_f64(ctx),
        1 => 0.0,
        2 => -0.0,
        3 => f64::INFINITY,
        4 => f64::NEG_INFINITY,
        // NaN はラウンドトリップで比較できないが、エンコード/デコード自体は可能
        _ => f64::NAN,
    }
}

/// AMF3 の Date 用の Duration をサンプリングする
fn sample_amf3_date(ctx: &mut TestCaseContext) -> Duration {
    // 非負で有限な値のみ (デコーダの制約)
    Duration::from_millis(noprop::sample_u64_in(ctx, 0..=253_402_300_799_999))
}

/// AMF3 のリーフ値 (再帰なし) をサンプリングする
fn sample_amf3_leaf(ctx: &mut TestCaseContext) -> Amf3Value {
    // 13 分岐を等確率で選択する
    match noprop::sample_weighted_index(ctx, &[1; 13]) {
        0 => Amf3Value::Undefined,
        1 => Amf3Value::Null,
        2 => Amf3Value::Boolean(noprop::sample_bool(ctx)),
        3 => Amf3Value::Integer(sample_amf3_integer(ctx)),
        4 => Amf3Value::Double(sample_amf3_double(ctx)),
        5 => Amf3Value::String(sample_amf3_string(ctx)),
        6 => Amf3Value::XmlDocument(sample_amf3_string(ctx)),
        7 => Amf3Value::Date {
            unix_time: sample_amf3_date(ctx),
        },
        8 => Amf3Value::Xml(sample_amf3_string(ctx)),
        9 => {
            let len = noprop::sample_usize_in(ctx, 0..100);
            Amf3Value::ByteArray(noprop::sample_bytes_vec(ctx, len))
        }
        // IntVector (32 ビット符号付き整数ベクタ)
        10 => {
            let is_fixed = noprop::sample_bool(ctx);
            let len = noprop::sample_usize_in(ctx, 0..10);
            let entries = (0..len).map(|_| noprop::sample_i32(ctx)).collect();
            Amf3Value::IntVector { is_fixed, entries }
        }
        // UintVector (32 ビット符号なし整数ベクタ)
        11 => {
            let is_fixed = noprop::sample_bool(ctx);
            let len = noprop::sample_usize_in(ctx, 0..10);
            let entries = (0..len).map(|_| noprop::sample_u32(ctx)).collect();
            Amf3Value::UintVector { is_fixed, entries }
        }
        // DoubleVector (64 ビット浮動小数点数ベクタ)
        _ => {
            let is_fixed = noprop::sample_bool(ctx);
            let len = noprop::sample_usize_in(ctx, 0..10);
            let entries = (0..len).map(|_| sample_amf3_double(ctx)).collect();
            Amf3Value::DoubleVector { is_fixed, entries }
        }
    }
}

/// キーと値の組を 0 個以上 max_len 個未満サンプリングする
///
/// `depth` は各値の子要素の深さを表す。
fn sample_amf3_pairs(
    ctx: &mut TestCaseContext,
    depth: usize,
    max_len: usize,
) -> Vec<Pair<String, Amf3Value>> {
    let len = noprop::sample_usize_in(ctx, 0..max_len);
    (0..len)
        .map(|_| {
            let key = sample_amf3_key(ctx);
            let value = sample_amf3_value(ctx, depth);
            Pair { key, value }
        })
        .collect()
}

/// AMF3 の値を 0 個以上 max_len 個未満サンプリングする
///
/// `depth` は各値の子要素の深さを表す。
fn sample_amf3_values(ctx: &mut TestCaseContext, depth: usize, max_len: usize) -> Vec<Amf3Value> {
    let len = noprop::sample_usize_in(ctx, 0..max_len);
    (0..len).map(|_| sample_amf3_value(ctx, depth)).collect()
}

/// Dictionary のキーと値の組を 0 個以上 max_len 個未満サンプリングする
///
/// 辞書のキーは任意の AMF3 値であるため、キーと値の両方を `depth` の深さで生成する。
fn sample_amf3_value_pairs(
    ctx: &mut TestCaseContext,
    depth: usize,
    max_len: usize,
) -> Vec<Pair<Amf3Value, Amf3Value>> {
    let len = noprop::sample_usize_in(ctx, 0..max_len);
    (0..len)
        .map(|_| {
            let key = sample_amf3_value(ctx, depth);
            let value = sample_amf3_value(ctx, depth);
            Pair { key, value }
        })
        .collect()
}

/// 深さ `depth` の AMF3 値をサンプリングする
///
/// `depth` が 0 なら必ずリーフ値を返す。それ以外は 50% の確率でリーフ値を返し、
/// 残りの 50% を 5 種類の複合値から等確率で選ぶ。子要素の深さは 1 だけ減らすため、
/// 再帰は `depth` で必ず止まり、総ノード数も深さと子要素数の上限で制限される。
fn sample_amf3_value(ctx: &mut TestCaseContext, depth: usize) -> Amf3Value {
    if depth == 0 || noprop::sample_bool(ctx) {
        return sample_amf3_leaf(ctx);
    }
    // 5 分岐を等確率で選択する
    match noprop::sample_weighted_index(ctx, &[1, 1, 1, 1, 1]) {
        // Array (連想配列のキーは空文字列不可)
        0 => Amf3Value::Array {
            assoc_entries: sample_amf3_pairs(ctx, depth - 1, 3),
            dense_entries: sample_amf3_values(ctx, depth - 1, 5),
        },
        // Object (無名、dynamic) - キーは空文字列不可
        1 => Amf3Value::Object {
            class_name: None,
            sealed_count: 0,
            entries: sample_amf3_pairs(ctx, depth - 1, 5),
        },
        // Object (名前付き、sealed + dynamic) - キーは空文字列不可
        2 => {
            let class_name = sample_amf3_key(ctx);
            let entries = sample_amf3_pairs(ctx, depth - 1, 5);
            // 先頭から半分を sealed member として扱う
            let sealed_count = entries.len() / 2;
            Amf3Value::Object {
                class_name: Some(class_name),
                sealed_count,
                entries,
            }
        }
        // ObjectVector (class_name "*" は「任意の型」を意味するので使用不可)
        3 => {
            let class_name = if noprop::sample_bool(ctx) {
                Some(sample_amf3_class_name(ctx))
            } else {
                None
            };
            let is_fixed = noprop::sample_bool(ctx);
            Amf3Value::ObjectVector {
                class_name,
                is_fixed,
                entries: sample_amf3_values(ctx, depth - 1, 5),
            }
        }
        // Dictionary (連想配列)
        _ => Amf3Value::Dictionary {
            is_weak: noprop::sample_bool(ctx),
            entries: sample_amf3_value_pairs(ctx, depth - 1, 5),
        },
    }
}

// =============================================================================
// AMF0 のプロパティ
// =============================================================================

/// AMF0 値のラウンドトリップテスト: エンコード → デコード → 元の値と一致
#[test]
fn amf0_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_amf0_value(ctx, MAX_DEPTH);

        // NaN を含む値は比較できないのでスキップする
        if contains_nan_amf0(&value) {
            return Ok(());
        }

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf0Value::decode(&encoded).expect("AMF0 のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        assert_eq!(decoded_value, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF0 Number の特殊値テスト
#[test]
fn amf0_number_special_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let n = sample_amf0_number(ctx);
        let value = Amf0Value::Number(n);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf0Value::decode(&encoded).expect("AMF0 Number のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );

        if let Amf0Value::Number(decoded_n) = decoded_value {
            if n.is_nan() {
                assert!(decoded_n.is_nan(), "NaN は NaN のままであるべき");
            } else {
                assert_eq!(
                    decoded_n.to_bits(),
                    n.to_bits(),
                    "Number のビット表現が一致しない"
                );
            }
        } else {
            panic!("デコード結果が Number ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF0 String の境界値テスト (短い文字列と長い文字列)
#[test]
fn amf0_string_boundary() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // String と LongString の境界値を等確率で選択する
        let len = noprop::sample_choice(ctx, &[0usize, 1, 0xFFFE, 0xFFFF, 0x10000, 0x10001]);
        let s: String = (0..len).map(|_| 'a').collect();
        let value = Amf0Value::String(s.clone());

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf0Value::decode(&encoded).expect("AMF0 String のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf0Value::String(decoded_s) = decoded_value {
            assert_eq!(decoded_s, s, "ラウンドトリップした文字列が一致しない");
        } else {
            panic!("デコード結果が String ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF0 Date のラウンドトリップテスト
#[test]
fn amf0_date_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let unix_time = sample_amf0_date(ctx);
        let value = Amf0Value::Date { unix_time };

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf0Value::decode(&encoded).expect("AMF0 Date のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf0Value::Date {
            unix_time: decoded_time,
        } = decoded_value
        {
            assert_eq!(
                decoded_time, unix_time,
                "ラウンドトリップした日時が一致しない"
            );
        } else {
            panic!("デコード結果が Date ではない");
        }
        Ok(())
    })?;
    Ok(())
}

// =============================================================================
// AMF3 のプロパティ
// =============================================================================

/// AMF3 値のラウンドトリップテスト: エンコード → デコード → 元の値と一致
#[test]
fn amf3_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let value = sample_amf3_value(ctx, MAX_DEPTH);

        // NaN を含む値は比較できないのでスキップする
        if contains_nan_amf3(&value) {
            return Ok(());
        }

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        assert_eq!(decoded_value, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Integer の境界値テスト (U29 エンコーディング)
#[test]
fn amf3_integer_boundary() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        // 正と負の境界値を等確率で選択する
        let n = noprop::sample_choice(
            ctx,
            &[
                0i32,       // ゼロ
                0x7F,       // 1 バイト上限
                0x80,       // 2 バイト下限
                0x3FFF,     // 2 バイト上限
                0x4000,     // 3 バイト下限
                0x1FFFFF,   // 3 バイト上限
                0x200000,   // 4 バイト下限
                268435455,  // i29 最大値
                -1,         // 負の最小絶対値
                -268435456, // i29 最小値
            ],
        );
        let value = Amf3Value::Integer(n);

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 Integer のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf3Value::Integer(decoded_n) = decoded_value {
            assert_eq!(
                decoded_n, n,
                "ラウンドトリップした Integer の値が一致しない"
            );
        } else {
            panic!("デコード結果が Integer ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Integer の範囲全体テスト
#[test]
fn amf3_integer_full_range() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let n = sample_amf3_integer(ctx);
        let value = Amf3Value::Integer(n);

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 Integer のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf3Value::Integer(decoded_n) = decoded_value {
            assert_eq!(
                decoded_n, n,
                "ラウンドトリップした Integer の値が一致しない"
            );
        } else {
            panic!("デコード結果が Integer ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Double の特殊値テスト
#[test]
fn amf3_double_special_values() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let n = sample_amf3_double(ctx);
        let value = Amf3Value::Double(n);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 Double のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );

        if let Amf3Value::Double(decoded_n) = decoded_value {
            if n.is_nan() {
                assert!(decoded_n.is_nan(), "NaN は NaN のままであるべき");
            } else {
                assert_eq!(
                    decoded_n.to_bits(),
                    n.to_bits(),
                    "Double のビット表現が一致しない"
                );
            }
        } else {
            panic!("デコード結果が Double ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 ByteArray のラウンドトリップテスト
#[test]
fn amf3_byte_array_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let len = noprop::sample_usize_in(ctx, 0..1000);
        let data = noprop::sample_bytes_vec(ctx, len);
        let value = Amf3Value::ByteArray(data.clone());

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 ByteArray のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf3Value::ByteArray(decoded_data) = decoded_value {
            assert_eq!(
                decoded_data, data,
                "ラウンドトリップしたバイト列が一致しない"
            );
        } else {
            panic!("デコード結果が ByteArray ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Vector 系のラウンドトリップテスト
#[test]
fn amf3_int_vector_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let is_fixed = noprop::sample_bool(ctx);
        let len = noprop::sample_usize_in(ctx, 0..100);
        let entries: Vec<i32> = (0..len).map(|_| noprop::sample_i32(ctx)).collect();
        let value = Amf3Value::IntVector {
            is_fixed,
            entries: entries.clone(),
        };

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 IntVector のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf3Value::IntVector {
            is_fixed: d_fixed,
            entries: d_entries,
        } = decoded_value
        {
            assert_eq!(d_fixed, is_fixed, "is_fixed が一致しない");
            assert_eq!(d_entries, entries, "ラウンドトリップした要素が一致しない");
        } else {
            panic!("デコード結果が IntVector ではない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Date のラウンドトリップテスト
#[test]
fn amf3_date_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(CASES, |ctx: &mut TestCaseContext| {
        let unix_time = sample_amf3_date(ctx);
        let value = Amf3Value::Date { unix_time };

        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            Amf3Value::decode(&encoded).expect("AMF3 Date のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        if let Amf3Value::Date {
            unix_time: decoded_time,
        } = decoded_value
        {
            assert_eq!(
                decoded_time, unix_time,
                "ラウンドトリップした日時が一致しない"
            );
        } else {
            panic!("デコード結果が Date ではない");
        }
        Ok(())
    })?;
    Ok(())
}

// =============================================================================
// AmfValue (wrapper) のプロパティ
// =============================================================================

/// AmfValue (AMF0) のラウンドトリップテスト
#[test]
fn amf_value_amf0_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(WRAPPER_CASES, |ctx: &mut TestCaseContext| {
        let value = sample_amf0_value(ctx, MAX_DEPTH);

        // NaN を含む値は比較できないのでスキップする
        if contains_nan_amf0(&value) {
            return Ok(());
        }

        let amf_value = AmfValue::Amf0(value);

        let mut encoded = Vec::new();
        amf_value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            AmfValue::decode(&encoded, AmfVersion::Amf0).expect("AmfValue のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        assert_eq!(
            decoded_value, amf_value,
            "ラウンドトリップした値が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

/// AmfValue (AMF3) のラウンドトリップテスト
#[test]
fn amf_value_amf3_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(WRAPPER_CASES, |ctx: &mut TestCaseContext| {
        let value = sample_amf3_value(ctx, MAX_DEPTH);

        // NaN を含む値は比較できないのでスキップする
        if contains_nan_amf3(&value) {
            return Ok(());
        }

        let amf_value = AmfValue::Amf3(value);

        let mut encoded = Vec::new();
        amf_value.encode(&mut encoded);

        let (decoded_size, decoded_value) =
            AmfValue::decode(&encoded, AmfVersion::Amf3).expect("AmfValue のデコードに失敗した");

        assert_eq!(
            decoded_size,
            encoded.len(),
            "デコードしたサイズが一致しない"
        );
        assert_eq!(
            decoded_value, amf_value,
            "ラウンドトリップした値が一致しない"
        );
        Ok(())
    })?;
    Ok(())
}

// =============================================================================
// AMF 仕様に基づく境界値テスト
// =============================================================================

// -----------------------------------------------------------------------------
// AMF0 仕様 Section 2: Type Markers のテスト
// -----------------------------------------------------------------------------

/// AMF0 仕様 Section 2.2: Number Type
///
/// IEEE-754 倍精度浮動小数点数 (8 バイト) であることを検証する。
#[test]
fn amf0_number_ieee754_compliance() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        // 正規化数、ゼロ、無限大、非正規化数の境界値を等確率で選択する
        let n = noprop::sample_choice(
            ctx,
            &[
                1.0f64,
                -1.0f64,
                f64::MIN,
                f64::MAX,
                f64::MIN_POSITIVE,
                0.0f64,
                -0.0f64,
                f64::INFINITY,
                f64::NEG_INFINITY,
                // 最小の正の非正規化数に近い値
                5e-324f64,
            ],
        );
        let value = Amf0Value::Number(n);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        // Number: 型マーカー (1 バイト) + DOUBLE (8 バイト) = 9 バイト
        assert_eq!(encoded.len(), 9, "AMF0 Number は 9 バイトになるべき");
        assert_eq!(
            encoded[0], 0x00,
            "AMF0 Number の型マーカーは 0x00 であるべき"
        );

        let (_, decoded) = Amf0Value::decode(&encoded).expect("AMF0 Number のデコードに失敗した");
        if let Amf0Value::Number(decoded_n) = decoded {
            assert_eq!(
                decoded_n.to_bits(),
                n.to_bits(),
                "Number のビット表現が一致しない"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF0 仕様 Section 2.3: Boolean Type
///
/// 型マーカー (1 バイト) + U8 (1 バイト) であることを検証する。
#[test]
fn amf0_boolean_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let b = noprop::sample_bool(ctx);
        let value = Amf0Value::Boolean(b);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        // Boolean: 型マーカー (1 バイト) + 値 (1 バイト) = 2 バイト
        assert_eq!(encoded.len(), 2, "AMF0 Boolean は 2 バイトになるべき");
        assert_eq!(
            encoded[0], 0x01,
            "AMF0 Boolean の型マーカーは 0x01 であるべき"
        );
        // 仕様: 0 は false、0 以外は true
        if b {
            assert_ne!(encoded[1], 0, "true は 0 以外で表現されるべき");
        } else {
            assert_eq!(encoded[1], 0, "false は 0 で表現されるべき");
        }

        let (_, decoded) = Amf0Value::decode(&encoded).expect("AMF0 Boolean のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF0 仕様 Section 2.4/2.14: String/LongString Type
///
/// String: 型マーカー + U16 長さ + UTF-8 文字列 (最大 65535 バイト)
/// LongString: 型マーカー + U32 長さ + UTF-8 文字列 (最大 4GB)
#[test]
fn amf0_string_encoding_size() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let len = noprop::sample_usize_in(ctx, 0..=100);
        let s: String = (0..len).map(|_| 'a').collect();
        let value = Amf0Value::String(s.clone());
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        if len <= 0xFFFF {
            // String: 型マーカー (1) + U16 長さ (2) + 文字列
            assert_eq!(encoded[0], 0x02, "String の型マーカーは 0x02 であるべき");
            assert_eq!(
                encoded.len(),
                1 + 2 + len,
                "String のエンコードサイズが一致しない"
            );
        } else {
            // LongString: 型マーカー (1) + U32 長さ (4) + 文字列
            assert_eq!(
                encoded[0], 0x0C,
                "LongString の型マーカーは 0x0C であるべき"
            );
            assert_eq!(
                encoded.len(),
                1 + 4 + len,
                "LongString のエンコードサイズが一致しない"
            );
        }

        let (_, decoded) = Amf0Value::decode(&encoded).expect("AMF0 String のデコードに失敗した");
        if let Amf0Value::String(decoded_s) = decoded {
            assert_eq!(decoded_s, s, "ラウンドトリップした文字列が一致しない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF0 仕様 Section 2.7: null Type
///
/// 型マーカー のみ (1 バイト) であることを検証する。
#[test]
fn amf0_null_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |_ctx: &mut TestCaseContext| {
        let value = Amf0Value::Null;
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(encoded.len(), 1, "AMF0 Null は 1 バイトになるべき");
        assert_eq!(encoded[0], 0x05, "AMF0 Null の型マーカーは 0x05 であるべき");

        let (_, decoded) = Amf0Value::decode(&encoded).expect("AMF0 Null のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF0 仕様 Section 2.8: undefined Type
///
/// 型マーカー のみ (1 バイト) であることを検証する。
#[test]
fn amf0_undefined_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |_ctx: &mut TestCaseContext| {
        let value = Amf0Value::Undefined;
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(encoded.len(), 1, "AMF0 Undefined は 1 バイトになるべき");
        assert_eq!(
            encoded[0], 0x06,
            "AMF0 Undefined の型マーカーは 0x06 であるべき"
        );

        let (_, decoded) =
            Amf0Value::decode(&encoded).expect("AMF0 Undefined のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF0 仕様 Section 2.13: Date Type
///
/// 型マーカー (1) + DOUBLE (8) + S16 タイムゾーン (2) = 11 バイト であることを検証する。
#[test]
fn amf0_date_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let unix_time = sample_amf0_date(ctx);
        let value = Amf0Value::Date { unix_time };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(encoded.len(), 11, "AMF0 Date は 11 バイトになるべき");
        assert_eq!(encoded[0], 0x0B, "AMF0 Date の型マーカーは 0x0B であるべき");

        let (_, decoded) = Amf0Value::decode(&encoded).expect("AMF0 Date のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

// -----------------------------------------------------------------------------
// AMF3 仕様 Section 1.3.1: U29 Variable Length Integer のテスト
// -----------------------------------------------------------------------------

/// AMF3 U29 エンコーディングのバイト数境界テスト
///
/// 仕様: 0x00-0x7F (1 バイト), 0x80-0x3FFF (2 バイト),
///       0x4000-0x1FFFFF (3 バイト), 0x200000-0x3FFFFFFF (4 バイト)
#[test]
fn amf3_u29_encoding_size() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        // 各エンコーディング幅の境界値を等確率で選択する
        let (n, expected_extra_bytes) = noprop::sample_choice(
            ctx,
            &[
                // 1 バイトエンコーディング: 0x00 - 0x7F
                (0i32, 1usize),
                (0x7F, 1),
                // 2 バイトエンコーディング: 0x80 - 0x3FFF
                (0x80, 2),
                (0x3FFF, 2),
                // 3 バイトエンコーディング: 0x4000 - 0x1FFFFF
                (0x4000, 3),
                (0x1FFFFF, 3),
                // 4 バイトエンコーディング: 0x200000 - 0x0FFFFFFF (i29 の正の最大値)
                (0x200000, 4),
                (268435455, 4),
            ],
        );
        let value = Amf3Value::Integer(n);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        // Integer: 型マーカー (1) + U29 (可変長)
        let expected_size = 1usize + expected_extra_bytes;
        assert_eq!(
            encoded.len(),
            expected_size,
            "AMF3 Integer {} は {} バイトになるべき",
            n,
            expected_size
        );
        assert_eq!(
            encoded[0], 0x04,
            "AMF3 Integer の型マーカーは 0x04 であるべき"
        );

        let (_, decoded) = Amf3Value::decode(&encoded).expect("AMF3 Integer のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF3 仕様 Section 3.4/3.5: Boolean Types (true/false)
///
/// 単独のマーカーのみ (それぞれ 1 byte) であることを検証する。
#[test]
fn amf3_boolean_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let b = noprop::sample_bool(ctx);
        let value = Amf3Value::Boolean(b);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(encoded.len(), 1, "AMF3 Boolean は 1 バイトになるべき");
        if b {
            assert_eq!(encoded[0], 0x03, "AMF3 true のマーカーは 0x03 であるべき");
        } else {
            assert_eq!(encoded[0], 0x02, "AMF3 false のマーカーは 0x02 であるべき");
        }

        let (_, decoded) = Amf3Value::decode(&encoded).expect("AMF3 Boolean のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF3 仕様 Section 3.2/3.3: undefined/null Types
#[test]
fn amf3_undefined_null_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let is_null = noprop::sample_bool(ctx);
        let value = if is_null {
            Amf3Value::Null
        } else {
            Amf3Value::Undefined
        };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded.len(),
            1,
            "AMF3 Undefined/Null は 1 バイトになるべき"
        );
        if is_null {
            assert_eq!(encoded[0], 0x01, "AMF3 null のマーカーは 0x01 であるべき");
        } else {
            assert_eq!(
                encoded[0], 0x00,
                "AMF3 undefined のマーカーは 0x00 であるべき"
            );
        }

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 Undefined/Null のデコードに失敗した");
        assert_eq!(decoded, value, "ラウンドトリップした値が一致しない");
        Ok(())
    })?;
    Ok(())
}

/// AMF3 仕様 Section 3.7: Double Type
///
/// 型マーカー (1) + IEEE-754 double (8) = 9 バイト であることを検証する。
#[test]
fn amf3_double_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        // 特殊値を含む境界値を等確率で選択する
        let n = noprop::sample_choice(
            ctx,
            &[
                0.0f64,
                -0.0f64,
                1.0f64,
                -1.0f64,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::MAX,
                f64::MIN,
            ],
        );
        let value = Amf3Value::Double(n);
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(encoded.len(), 9, "AMF3 Double は 9 バイトになるべき");
        assert_eq!(
            encoded[0], 0x05,
            "AMF3 Double の型マーカーは 0x05 であるべき"
        );

        let (_, decoded) = Amf3Value::decode(&encoded).expect("AMF3 Double のデコードに失敗した");
        if let Amf3Value::Double(decoded_n) = decoded {
            assert_eq!(
                decoded_n.to_bits(),
                n.to_bits(),
                "Double のビット表現が一致しない"
            );
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 仕様 Section 3.14: ByteArray Type
///
/// 型マーカー (1) + U29B 値 + データ であることを検証する。
#[test]
fn amf3_byte_array_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let len = noprop::sample_usize_in(ctx, 0..50);
        let data = noprop::sample_bytes_vec(ctx, len);
        let value = Amf3Value::ByteArray(data.clone());
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded[0], 0x0C,
            "AMF3 ByteArray の型マーカーは 0x0C であるべき"
        );

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 ByteArray のデコードに失敗した");
        if let Amf3Value::ByteArray(decoded_data) = decoded {
            assert_eq!(
                decoded_data, data,
                "ラウンドトリップしたバイト列が一致しない"
            );
        }
        Ok(())
    })?;
    Ok(())
}

// -----------------------------------------------------------------------------
// AMF3 仕様 Section 3.15: Vector Types のテスト
// -----------------------------------------------------------------------------

/// AMF3 Vector.<int> テスト
#[test]
fn amf3_int_vector_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let is_fixed = noprop::sample_bool(ctx);
        let len = noprop::sample_usize_in(ctx, 0..20);
        let entries: Vec<i32> = (0..len).map(|_| noprop::sample_i32(ctx)).collect();
        let value = Amf3Value::IntVector {
            is_fixed,
            entries: entries.clone(),
        };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded[0], 0x0D,
            "AMF3 Vector.<int> の型マーカーは 0x0D であるべき"
        );

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 Vector.<int> のデコードに失敗した");
        if let Amf3Value::IntVector {
            is_fixed: d_fixed,
            entries: d_entries,
        } = decoded
        {
            assert_eq!(d_fixed, is_fixed, "is_fixed が一致しない");
            assert_eq!(d_entries, entries, "ラウンドトリップした要素が一致しない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Vector.<uint> テスト
#[test]
fn amf3_uint_vector_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let is_fixed = noprop::sample_bool(ctx);
        let len = noprop::sample_usize_in(ctx, 0..20);
        let entries: Vec<u32> = (0..len).map(|_| noprop::sample_u32(ctx)).collect();
        let value = Amf3Value::UintVector {
            is_fixed,
            entries: entries.clone(),
        };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded[0], 0x0E,
            "AMF3 Vector.<uint> の型マーカーは 0x0E であるべき"
        );

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 Vector.<uint> のデコードに失敗した");
        if let Amf3Value::UintVector {
            is_fixed: d_fixed,
            entries: d_entries,
        } = decoded
        {
            assert_eq!(d_fixed, is_fixed, "is_fixed が一致しない");
            assert_eq!(d_entries, entries, "ラウンドトリップした要素が一致しない");
        }
        Ok(())
    })?;
    Ok(())
}

/// AMF3 Vector.<Number> テスト (NaN を除外)
#[test]
fn amf3_double_vector_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let is_fixed = noprop::sample_bool(ctx);
        let len = noprop::sample_usize_in(ctx, 0..20);
        // NaN を除外した境界値のみを等確率で選択する
        let entries: Vec<f64> = (0..len)
            .map(|_| noprop::sample_choice(ctx, &[0.0f64, 1.0f64, -1.0f64]))
            .collect();
        let value = Amf3Value::DoubleVector {
            is_fixed,
            entries: entries.clone(),
        };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded[0], 0x0F,
            "AMF3 Vector.<Number> の型マーカーは 0x0F であるべき"
        );

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 Vector.<Number> のデコードに失敗した");
        if let Amf3Value::DoubleVector {
            is_fixed: d_fixed,
            entries: d_entries,
        } = decoded
        {
            assert_eq!(d_fixed, is_fixed, "is_fixed が一致しない");
            assert_eq!(d_entries, entries, "ラウンドトリップした要素が一致しない");
        }
        Ok(())
    })?;
    Ok(())
}

// -----------------------------------------------------------------------------
// AMF3 仕様 Section 3.16: Dictionary Type のテスト
// -----------------------------------------------------------------------------

/// AMF3 Dictionary テスト
#[test]
fn amf3_dictionary_encoding() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    noprop::Runner::new(seed).run(ENCODING_CASES, |ctx: &mut TestCaseContext| {
        let is_weak = noprop::sample_bool(ctx);
        // 簡単な Dictionary (キーと値のペア)
        let entries = vec![
            Pair {
                key: Amf3Value::String("key1".to_string()),
                value: Amf3Value::Integer(100),
            },
            Pair {
                key: Amf3Value::Integer(42),
                value: Amf3Value::String("value2".to_string()),
            },
        ];
        let value = Amf3Value::Dictionary {
            is_weak,
            entries: entries.clone(),
        };
        let mut encoded = Vec::new();
        value.encode(&mut encoded);

        assert_eq!(
            encoded[0], 0x11,
            "AMF3 Dictionary の型マーカーは 0x11 であるべき"
        );

        let (_, decoded) =
            Amf3Value::decode(&encoded).expect("AMF3 Dictionary のデコードに失敗した");
        if let Amf3Value::Dictionary {
            is_weak: d_weak,
            entries: d_entries,
        } = decoded
        {
            assert_eq!(d_weak, is_weak, "is_weak が一致しない");
            assert_eq!(
                d_entries.len(),
                entries.len(),
                "ラウンドトリップした要素数が一致しない"
            );
        }
        Ok(())
    })?;
    Ok(())
}

// =============================================================================
// ヘルパー関数
// =============================================================================

/// AMF0 値が NaN を含むかどうかをチェックする
fn contains_nan_amf0(value: &Amf0Value) -> bool {
    match value {
        Amf0Value::Number(n) => n.is_nan(),
        Amf0Value::Object { entries, .. } => entries.iter().any(|p| contains_nan_amf0(&p.value)),
        Amf0Value::EcmaArray { entries } => entries.iter().any(|p| contains_nan_amf0(&p.value)),
        Amf0Value::Array { entries } => entries.iter().any(contains_nan_amf0),
        Amf0Value::AvmPlus(v) => contains_nan_amf3(v),
        _ => false,
    }
}

/// AMF3 値が NaN を含むかどうかをチェックする
fn contains_nan_amf3(value: &Amf3Value) -> bool {
    match value {
        Amf3Value::Double(n) => n.is_nan(),
        Amf3Value::Array {
            assoc_entries,
            dense_entries,
        } => {
            assoc_entries.iter().any(|p| contains_nan_amf3(&p.value))
                || dense_entries.iter().any(contains_nan_amf3)
        }
        Amf3Value::Object { entries, .. } => entries.iter().any(|p| contains_nan_amf3(&p.value)),
        Amf3Value::DoubleVector { entries, .. } => entries.iter().any(|n| n.is_nan()),
        Amf3Value::ObjectVector { entries, .. } => entries.iter().any(contains_nan_amf3),
        Amf3Value::Dictionary { entries, .. } => entries
            .iter()
            .any(|p| contains_nan_amf3(&p.key) || contains_nan_amf3(&p.value)),
        _ => false,
    }
}
