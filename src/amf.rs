use crate::amf0::Amf0Value;
use crate::amf3::Amf3Value;
use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum AmfVersion {
    Amf0,
    Amf3,
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AmfValue {
    Amf0(Amf0Value),
    Amf3(Amf3Value),
}

impl AmfValue {
    pub fn decode(buf: &[u8], version: AmfVersion) -> Result<(usize, Self), Error> {
        match version {
            AmfVersion::Amf0 => Amf0Value::decode(buf).map(|(n, v)| (n, Self::Amf0(v))),
            AmfVersion::Amf3 => Amf3Value::decode(buf).map(|(n, v)| (n, Self::Amf3(v))),
        }
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Amf0(x) => x.encode(buf),
            Self::Amf3(x) => x.encode(buf),
        }
    }

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

    pub fn expect_str(&self) -> Result<&str, Error> {
        self.to_ref().expect_str()
    }

    pub fn expect_number(&self) -> Result<f64, Error> {
        self.to_ref().expect_number()
    }

    fn to_ref(&self) -> AmfValueRef<'_> {
        match self {
            Self::Amf0(v) => AmfValueRef::Amf0(v),
            Self::Amf3(v) => AmfValueRef::Amf3(v),
        }
    }

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

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum AmfValueRef<'a> {
    Amf0(&'a Amf0Value),
    Amf3(&'a Amf3Value),
}

impl<'a> AmfValueRef<'a> {
    pub fn expect_str(&self) -> Result<&'a str, Error> {
        match self {
            Self::Amf0(Amf0Value::String(s)) => Ok(s),
            Self::Amf3(Amf3Value::String(s)) => Ok(s),
            _ => Err(Error::invalid_data("value is not an AMF string")),
        }
    }

    pub fn expect_number(&self) -> Result<f64, Error> {
        match self {
            Self::Amf0(Amf0Value::Number(n)) => Ok(*n),
            Self::Amf3(Amf3Value::Integer(n)) => Ok(*n as f64),
            Self::Amf3(Amf3Value::Double(n)) => Ok(*n),
            _ => Err(Error::invalid_data("value is not an AMF number")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pair<K, V> {
    pub key: K,
    pub value: V,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ErrorKind;

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
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
                assert_eq!(err.kind, ErrorKind::InsufficientBuffer);
                continue;
            }
            if test_file_name.contains("-bad-") {
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
                assert_eq!(err.kind, ErrorKind::InvalidData);
                continue;
            }
            if test_file_name.contains("-unsupported-") {
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
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
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
                assert_eq!(err.kind, ErrorKind::InsufficientBuffer);
                continue;
            }
            if test_file_name.contains("-bad-") {
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
                assert_eq!(err.kind, ErrorKind::InvalidData);
                continue;
            }
            if test_file_name.contains("-unsupported-") {
                let err = AmfValue::decode(&original_data, version)
                    .expect_err("AmfValue::decode() success");
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
}
