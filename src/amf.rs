//! AMF0 と AMF3 の値を共通の型で扱い、バージョンに応じた変換を提供するモジュール

use alloc::borrow::ToOwned;
use alloc::vec::Vec;

use crate::amf0::Amf0Value;
use crate::amf3::Amf3Value;
use crate::error::Error;

/// AMF のバージョン
///
/// AMF0 と AMF3 のどちらの形式で値を解釈・出力するかを表す。
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum AmfVersion {
    /// AMF0（Action Message Format 0）形式
    Amf0,

    /// AMF3（Action Message Format 3）形式
    Amf3,
}

/// AMF0 と AMF3 のどちらかの値を保持する共通の値型
///
/// 保持している値のバージョンは variant で区別する。
/// [`AmfValue::decode`] は引数で指定したバージョンとしてバイト列を解釈し、
/// [`AmfValue::encode`] は保持している variant のバージョンの形式で出力する。
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AmfValue {
    /// AMF0 の値
    Amf0(Amf0Value),

    /// AMF3 の値
    Amf3(Amf3Value),
}

impl AmfValue {
    /// バイト列の先頭から AMF の値を 1 つデコードする
    ///
    /// `version` で指定したバージョンの形式として解釈し、デコードに使用したバイト数と値の組を返す。
    ///
    /// # エラー
    ///
    /// 次の場合はエラーを返す。
    ///
    /// - 入力が途中で終わっている場合
    /// - 未知または未対応の型マーカーが現れた場合
    /// - 型マーカーに対応するデータが不正である場合（不正な UTF-8、範囲外の参照番号、不正な日時の値など）
    /// - 循環参照など、この実装が対応していない表現が現れた場合
    pub fn decode(buf: &[u8], version: AmfVersion) -> Result<(usize, Self), Error> {
        match version {
            AmfVersion::Amf0 => Amf0Value::decode(buf).map(|(n, v)| (n, Self::Amf0(v))),
            AmfVersion::Amf3 => Amf3Value::decode(buf).map(|(n, v)| (n, Self::Amf3(v))),
        }
    }

    /// AMF の値をエンコードして `buf` の末尾に追記する
    ///
    /// 保持している値のバージョン（`Amf0` なら AMF0、`Amf3` なら AMF3）の形式で出力する。
    ///
    /// # パニック
    ///
    /// AMF3 の整数（`Amf3Value::Integer`）が `-2^28` より大きく `2^28` 未満の範囲に
    /// 収まっていない場合はパニックする。
    pub fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Amf0(x) => x.encode(buf),
            Self::Amf3(x) => x.encode(buf),
        }
    }

    /// 自身が AMF オブジェクトである場合に、指定したキーのメンバー値への参照を返す
    ///
    /// 同じキーを持つメンバーが複数ある場合は、最初に見つかったものを返す。
    /// 戻り値は自身が保持する値を借用する。
    ///
    /// # エラー
    ///
    /// 次の場合はエラーを返す。
    ///
    /// - 自身が AMF0 または AMF3 のオブジェクトでない場合
    /// - 自身がオブジェクトだが、`key` を持つメンバーが存在しない場合
    #[track_caller]
    pub fn expect_object_member(&self, key: &str) -> Result<AmfValueRef<'_>, Error> {
        match self {
            Self::Amf0(Amf0Value::Object { entries, .. }) => entries
                .iter()
                .find(|pair| pair.key == key)
                .map(|pair| AmfValueRef::Amf0(&pair.value))
                .ok_or_else(|| Error::invalid_data(format!("missing required key: {key}"))),
            Self::Amf3(Amf3Value::Object { entries, .. }) => entries
                .iter()
                .find(|pair| pair.key == key)
                .map(|pair| AmfValueRef::Amf3(&pair.value))
                .ok_or_else(|| Error::invalid_data(format!("missing required key: {key}"))),
            _ => Err(Error::invalid_data("value is not an AMF object")),
        }
    }

    /// 自身が文字列である場合に、その文字列を返す
    ///
    /// # エラー
    ///
    /// 自身が AMF0 の文字列でも AMF3 の文字列でもない場合はエラーを返す。
    #[track_caller]
    pub fn expect_str(&self) -> Result<&str, Error> {
        self.to_ref().expect_str()
    }

    /// 自身が数値である場合に、`f64` に変換して返す
    ///
    /// AMF3 の整数（`Integer`）は `f64` に変換して返す。
    ///
    /// # エラー
    ///
    /// 自身が AMF0 の数値（`Number`）、AMF3 の整数（`Integer`）、
    /// AMF3 の倍精度浮動小数点数（`Double`）のいずれでもない場合はエラーを返す。
    #[track_caller]
    pub fn expect_number(&self) -> Result<f64, Error> {
        self.to_ref().expect_number()
    }

    fn to_ref(&self) -> AmfValueRef<'_> {
        match self {
            Self::Amf0(v) => AmfValueRef::Amf0(v),
            Self::Amf3(v) => AmfValueRef::Amf3(v),
        }
    }

    /// AMF0 の無名オブジェクトを生成する
    ///
    /// `entries` の各キーと値の組をメンバーとして持つ AMF0 のオブジェクトを返す。
    /// クラス名は設定しないため、無名オブジェクトとしてエンコードされる。
    /// キーは所有権を持つ文字列にコピーされる。
    pub fn amf0_object<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = (&'a str, Amf0Value)>,
    {
        Self::Amf0(Amf0Value::Object {
            class_name: None,
            entries: entries
                .into_iter()
                .map(|(k, v)| Pair {
                    key: k.to_owned(),
                    value: v,
                })
                .collect(),
        })
    }
}

impl From<(AmfVersion, &str)> for AmfValue {
    fn from((version, value): (AmfVersion, &str)) -> Self {
        match version {
            AmfVersion::Amf0 => Self::Amf0(Amf0Value::String(value.to_owned())),
            AmfVersion::Amf3 => Self::Amf3(Amf3Value::String(value.to_owned())),
        }
    }
}

impl From<(AmfVersion, f64)> for AmfValue {
    fn from((version, value): (AmfVersion, f64)) -> Self {
        match version {
            AmfVersion::Amf0 => Self::Amf0(Amf0Value::Number(value)),
            AmfVersion::Amf3 => Self::Amf3(Amf3Value::Double(value)),
        }
    }
}

/// AMF0 または AMF3 の値への参照
///
/// 値を複製せずに参照するための型で、AMF オブジェクトのメンバー値を取り出す際などに使用する。
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AmfValueRef<'a> {
    /// AMF0 の値への参照
    Amf0(&'a Amf0Value),

    /// AMF3 の値への参照
    Amf3(&'a Amf3Value),
}

impl<'a> AmfValueRef<'a> {
    /// 参照先が文字列である場合に、その文字列を返す
    ///
    /// 戻り値は参照先と同じ有効期間を持つ。
    ///
    /// # エラー
    ///
    /// 参照先が AMF0 または AMF3 の文字列でない場合はエラーを返す。
    pub fn expect_str(&self) -> Result<&'a str, Error> {
        match self {
            Self::Amf0(Amf0Value::String(s)) => Ok(s),
            Self::Amf3(Amf3Value::String(s)) => Ok(s),
            _ => Err(Error::invalid_data("value is not an AMF string")),
        }
    }

    /// 参照先が数値である場合に、`f64` に変換して返す
    ///
    /// AMF3 の整数（`Integer`）は `f64` に変換して返す。
    ///
    /// # エラー
    ///
    /// 参照先が AMF0 の数値（`Number`）、AMF3 の整数（`Integer`）、
    /// AMF3 の倍精度浮動小数点数（`Double`）のいずれでもない場合はエラーを返す。
    pub fn expect_number(&self) -> Result<f64, Error> {
        match self {
            Self::Amf0(Amf0Value::Number(n)) => Ok(*n),
            Self::Amf3(Amf3Value::Integer(n)) => Ok(*n as f64),
            Self::Amf3(Amf3Value::Double(n)) => Ok(*n),
            _ => Err(Error::invalid_data("value is not an AMF number")),
        }
    }
}

/// キーと値の組
///
/// AMF0 のオブジェクト・ECMA 配列や、AMF3 のオブジェクト・配列・辞書のメンバーを表す。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pair<K, V> {
    /// メンバーのキー
    pub key: K,

    /// メンバーの値
    pub value: V,
}
