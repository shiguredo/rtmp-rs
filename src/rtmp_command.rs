//! RTMP のコマンドメッセージを扱うモジュール。
//! 各コマンドの型定義、AMF 値との相互変換、および応答メッセージの生成を提供する。

use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::iter;

use crate::amf::{AmfValue, AmfVersion};
use crate::amf0::Amf0Value;
use crate::error::Error;
use crate::rtmp_message::{RtmpMessage, RtmpMessageHeader, RtmpMessageStreamId};
use crate::rtmp_timestamp::RtmpTimestamp;

/// RTMP のコマンドメッセージで使用されるトランザクション ID
///
/// RTMP 仕様 7.2 節では、送信側がコマンド名・トランザクション ID・コマンドオブジェクトから
/// なるコマンドメッセージを送り、受信側は同じトランザクション ID を付けて応答すると
/// 定義されている。この型はそのトランザクション ID を表す。
///
/// # 補足
///
/// AMF の表現力的には f64 で保持するのが正しいが、RTMP の仕様では小数は推奨されておらず、
/// 実際に使われることもないため、内部的には整数で保持している。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TransactionId(i64);

impl TransactionId {
    /// onStatus コマンドで使用されるトランザクション ID（0）
    pub const ON_STATUS: Self = Self(0);

    /// connect コマンドで使用されるトランザクション ID（1）
    pub const CONNECT: Self = Self(1);

    /// アプリケーションが自由に使用できるトランザクション ID の開始値（2）
    pub const NON_RESERVED_START: Self = Self(2);

    /// AMF の数値からトランザクション ID を生成する
    ///
    /// 小数部分は切り捨てる。RTMP では実質的に整数のみが使われるため。
    pub fn from_f64(id: f64) -> Self {
        // `no_std` では `f64::round` が使えない。RTMP では実質整数のみのため切り捨てでよい
        Self(id as i64)
    }

    /// トランザクション ID を i64 の値として取得する
    pub const fn get(self) -> i64 {
        self.0
    }

    /// トランザクション ID を 1 増やす
    pub fn increment(&mut self) {
        self.0 += 1
    }
}

/// RTMP のコマンドメッセージを表す enum
///
/// 受信したコマンドメッセージをコマンド名に応じた型で保持する。
/// クレートが明示的に扱わないコマンドは [`RtmpCommand::Ignore`] として保持する。
#[derive(Debug, Clone)]
pub enum RtmpCommand {
    /// connect コマンド（RTMP 仕様 7.2.1.1 節）
    Connect(RtmpConnectCommand),

    /// createStream コマンド（RTMP 仕様 7.2.1.3 節）
    CreateStream(RtmpCreateStreamCommand),

    /// publish コマンド（RTMP 仕様 7.2.2.6 節）
    Publish(RtmpPublishCommand),

    /// play コマンド（RTMP 仕様 7.2.2.1 節）
    Play(RtmpPlayCommand),

    /// deleteStream コマンド（RTMP 仕様 7.2.2.3 節）
    DeleteStream(RtmpDeleteStreamCommand),

    /// getStreamLength コマンド
    GetStreamLength(RtmpGetStreamLengthCommand),

    /// コマンドに対する正常応答を表す _result コマンド（RTMP 仕様 7.2 節）
    Result(RtmpResultCommand),

    /// ストリームや接続の状態変化を通知する onStatus コマンド
    ///
    /// RTMP 仕様 7.2.2 節で NetStream の状態通知として定義されている。
    /// Enhanced RTMP 仕様では、NetConnection の再接続要求などの通知にも使用される。
    OnStatus(RtmpOnStatusCommand),

    /// クレートが明示的に扱わないコマンド
    ///
    /// 受信したコマンドの名前と AMF 値の組をそのまま保持する。
    Ignore {
        /// コマンド名
        name: String,

        // 以降のフィールドはデバッグ表示用で、クレート内のコードからは参照されない
        // （`tests` モジュール経由で PBT / fuzzing から参照されるため dead_code にはならない）
        /// コマンドオブジェクト
        object: AmfValue,

        /// コマンドの引数
        args: Vec<AmfValue>,
    },
}

impl RtmpCommand {
    /// コマンド名を返す
    ///
    /// [`RtmpCommand::Ignore`] の場合は、保持している受信時のコマンド名をそのまま返す。
    pub fn name(&self) -> &str {
        match self {
            RtmpCommand::Connect(_) => "connect",
            RtmpCommand::CreateStream(_) => "createStream",
            RtmpCommand::Publish(_) => "publish",
            RtmpCommand::Play(_) => "play",
            RtmpCommand::DeleteStream(_) => "deleteStream",
            RtmpCommand::GetStreamLength(_) => "getStreamLength",
            RtmpCommand::Result(_) => "_result",
            RtmpCommand::OnStatus(_) => "onStatus",
            RtmpCommand::Ignore { name, .. } => name,
        }
    }

    /// コマンドを AMF0 のコマンドメッセージに変換する
    ///
    /// 指定されたヘッダーを使って [`RtmpMessage::Command`] を組み立てる。
    /// [`RtmpCommand::Ignore`] は送信可能なコマンドとして表現できないため、エラーを返す。
    ///
    /// # 引数
    ///
    /// * `header` - 変換後のメッセージに設定するヘッダー
    pub fn into_message(self, header: RtmpMessageHeader) -> Result<RtmpMessage, Error> {
        match self {
            RtmpCommand::Ignore { .. } => {
                // ここに来るのは想定外（実装バグ）
                Err(Error::invalid_state("BUG"))
            }
            RtmpCommand::OnStatus(cmd) => {
                let mut pairs = vec![
                    ("level".to_string(), Amf0Value::String(cmd.level)),
                    ("code".to_string(), Amf0Value::String(cmd.code)),
                ];

                if let Some(description) = cmd.description {
                    pairs.push(("description".to_string(), Amf0Value::String(description)));
                }

                if let Some(details) = cmd.details {
                    pairs.push(("details".to_string(), Amf0Value::String(details)));
                }

                let status_object = AmfValue::Amf0(Amf0Value::Object {
                    class_name: None,
                    entries: pairs
                        .into_iter()
                        .map(|(k, v)| crate::amf::Pair { key: k, value: v })
                        .collect(),
                });

                Ok(RtmpMessage::Command {
                    header,
                    amf_version: AmfVersion::Amf0,
                    name: "onStatus".to_string(),
                    transaction_id: TransactionId::ON_STATUS,
                    object: AmfValue::Amf0(Amf0Value::Null),
                    args: vec![status_object],
                })
            }
            RtmpCommand::Connect(cmd) => {
                let object = AmfValue::amf0_object([
                    ("app", Amf0Value::String(cmd.app)),
                    ("flashVer", Amf0Value::String(cmd.flash_ver)),
                    ("tcUrl", Amf0Value::String(cmd.tc_url)),
                ]);
                Ok(RtmpMessage::Command {
                    header,
                    amf_version: AmfVersion::Amf0,
                    name: "connect".to_string(),
                    transaction_id: TransactionId::CONNECT,
                    object,
                    args: vec![],
                })
            }
            RtmpCommand::CreateStream(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "createStream".to_string(),
                transaction_id: cmd.transaction_id,
                object: AmfValue::Amf0(Amf0Value::Null),
                args: vec![],
            }),
            RtmpCommand::Publish(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "publish".to_string(),
                transaction_id: cmd.transaction_id,
                object: AmfValue::Amf0(Amf0Value::Null),
                args: vec![
                    AmfValue::Amf0(Amf0Value::String(cmd.stream_name)),
                    AmfValue::Amf0(Amf0Value::String("live".to_owned())), // 配信種別は live 固定
                ],
            }),
            RtmpCommand::Play(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "play".to_string(),
                transaction_id: cmd.transaction_id,
                object: AmfValue::Amf0(Amf0Value::Null),
                args: vec![
                    AmfValue::Amf0(Amf0Value::String(cmd.stream_name)),
                    AmfValue::Amf0(Amf0Value::Number(cmd.start)),
                ],
            }),
            RtmpCommand::DeleteStream(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "deleteStream".to_string(),
                transaction_id: cmd.transaction_id,
                object: AmfValue::Amf0(Amf0Value::Null),
                args: vec![AmfValue::Amf0(
                    Amf0Value::Number(cmd.stream_id.get() as f64),
                )],
            }),
            RtmpCommand::GetStreamLength(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "getStreamLength".to_string(),
                transaction_id: cmd.transaction_id,
                object: AmfValue::Amf0(Amf0Value::Null),
                args: vec![AmfValue::Amf0(Amf0Value::String(cmd.stream_name))],
            }),
            RtmpCommand::Result(cmd) => Ok(RtmpMessage::Command {
                header,
                amf_version: AmfVersion::Amf0,
                name: "_result".to_string(),
                transaction_id: cmd.transaction_id,
                object: cmd.properties,
                args: vec![cmd.information],
            }),
        }
    }

    /// 制御用ストリーム宛てのコマンドメッセージに変換する
    ///
    /// ストリーム ID 0、タイムスタンプ 0 のヘッダーで [`RtmpCommand::into_message`] を呼び出す。
    /// [`RtmpCommand::Ignore`] の場合はエラーを返す。
    pub fn into_pcm_message(self) -> Result<RtmpMessage, Error> {
        self.into_message(RtmpMessageHeader {
            stream_id: RtmpMessageStreamId::PCM,
            timestamp: RtmpTimestamp::ZERO,
        })
    }

    /// 受信したコマンドメッセージの構成要素からコマンドを生成する
    ///
    /// コマンド名に応じて対応する型に変換する。クレートが明示的に扱わないコマンド名の場合は
    /// [`RtmpCommand::Ignore`] として返す。必須の引数が不足している場合や型が不正な場合は
    /// エラーを返す。省略可能な引数は、コマンドごとの既定値を使う。
    ///
    /// # 引数
    ///
    /// * `name` - コマンド名
    /// * `transaction_id` - コマンドのトランザクション ID
    /// * `object` - コマンドオブジェクト
    /// * `args` - コマンドの引数
    pub fn from_message(
        name: &str,
        transaction_id: TransactionId,
        object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        match name {
            "connect" => {
                RtmpConnectCommand::from_message(transaction_id, object).map(Self::Connect)
            }
            "createStream" => RtmpCreateStreamCommand::from_message(transaction_id, object)
                .map(Self::CreateStream),
            "publish" => {
                RtmpPublishCommand::from_message(transaction_id, object, args).map(Self::Publish)
            }
            "play" => RtmpPlayCommand::from_message(transaction_id, object, args).map(Self::Play),
            "deleteStream" => RtmpDeleteStreamCommand::from_message(transaction_id, object, args)
                .map(Self::DeleteStream),
            "getStreamLength" => {
                RtmpGetStreamLengthCommand::from_message(transaction_id, object, args)
                    .map(Self::GetStreamLength)
            }
            "_result" => {
                RtmpResultCommand::from_message(transaction_id, object, args).map(Self::Result)
            }
            "onStatus" => {
                RtmpOnStatusCommand::from_message(transaction_id, object, args).map(Self::OnStatus)
            }
            _ => {
                // このクレート内で明示的にハンドリングしないものは全て Ignore 扱いにする
                Ok(Self::Ignore {
                    name: name.to_string(),
                    object,
                    args,
                })
            }
        }
    }
}

/// connect コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.1.1 節で定義される NetConnection の connect コマンドに対応する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpConnectCommand {
    /// 接続先のアプリケーション名
    pub app: String,

    /// クライアントの Flash Player のバージョン
    pub flash_ver: String,

    /// 接続先の URL
    pub tc_url: String,
}

impl RtmpConnectCommand {
    fn from_message(transaction_id: TransactionId, object: AmfValue) -> Result<Self, Error> {
        if transaction_id != TransactionId::CONNECT {
            return Err(Error::invalid_data(format!(
                "invalid transaction ID for connect command: expected {}, got {}",
                TransactionId::CONNECT.get(),
                transaction_id.get()
            )));
        }

        let app = object
            .expect_object_member("app")?
            .expect_str()?
            .to_string();
        let flash_ver = object
            .expect_object_member("flashVer")?
            .expect_str()?
            .to_string();
        let tc_url = object
            .expect_object_member("tcUrl")?
            .expect_str()?
            .to_string();
        Ok(Self {
            app,
            flash_ver,
            tc_url,
        })
    }

    /// connect コマンドを受け入れたことを示す _result メッセージを生成する
    ///
    /// 接続成功を表す `NetConnection.Connect.Success` を情報オブジェクトに含め、
    /// 制御用ストリーム宛てのメッセージとして返す。
    pub fn accept(&self) -> Result<RtmpMessage, Error> {
        let properties = AmfValue::amf0_object([
            ("fmsVer", Amf0Value::String("FMS/4,5,0,297".to_string())),
            ("capabilities", Amf0Value::Number(255.0)),
            ("mode", Amf0Value::Number(1.0)),
        ]);
        let information = AmfValue::amf0_object([
            ("level", Amf0Value::String("status".to_string())),
            (
                "code",
                Amf0Value::String("NetConnection.Connect.Success".to_string()),
            ),
            (
                "description",
                Amf0Value::String("Connection succeeded.".to_string()),
            ),
            ("objectEncoding", Amf0Value::Number(0.0)),
        ]);
        let command = RtmpResultCommand {
            transaction_id: TransactionId::CONNECT,
            properties,
            information,
        };
        RtmpCommand::Result(command).into_pcm_message()
    }
}

/// createStream コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.1.3 節で定義される NetConnection の createStream コマンドに対応する。
#[derive(Debug, Clone)]
pub struct RtmpCreateStreamCommand {
    /// コマンドのトランザクション ID
    pub transaction_id: TransactionId,
}

impl RtmpCreateStreamCommand {
    fn from_message(transaction_id: TransactionId, _object: AmfValue) -> Result<Self, Error> {
        Ok(Self { transaction_id })
    }
}

/// publish コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.2.6 節で定義される NetStream の publish コマンドに対応する。
/// クレートは配信種別として live のみを扱う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpPublishCommand {
    /// コマンドのトランザクション ID
    pub transaction_id: TransactionId,

    /// 配信するストリーム名
    pub stream_name: String,
}

impl RtmpPublishCommand {
    fn from_message(
        transaction_id: TransactionId,
        _object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let stream_name = args
            .first()
            .ok_or_else(|| Error::invalid_data("publish: missing stream name"))?
            .expect_str()?
            .to_string();
        let publish_type = args
            .get(1)
            .ok_or_else(|| Error::invalid_data("publish: missing publish type"))?
            .expect_str()?
            .to_string();
        if publish_type != "live" {
            return Err(Error::unsupported(format!(
                "unsupported publish type: {publish_type} (only 'live' is supported)",
            )));
        }

        Ok(Self {
            transaction_id,
            stream_name,
        })
    }

    /// publish コマンドを受け入れたことを示す _result メッセージを生成する
    ///
    /// 配信開始を表す `NetStream.Publish.Start` を情報オブジェクトに含め、
    /// 指定されたストリームの ID のヘッダーで返す。
    pub fn accept(
        transaction_id: TransactionId,
        stream_id: RtmpMessageStreamId,
    ) -> Result<RtmpMessage, Error> {
        let properties = AmfValue::amf0_object(iter::empty());
        let information = AmfValue::amf0_object([
            ("level", Amf0Value::String("status".to_string())),
            (
                "code",
                Amf0Value::String("NetStream.Publish.Start".to_string()),
            ),
            (
                "description",
                Amf0Value::String("Publish succeeded.".to_string()),
            ),
        ]);
        let command = RtmpResultCommand {
            transaction_id,
            properties,
            information,
        };
        RtmpCommand::Result(command).into_message(RtmpMessageHeader {
            stream_id,
            timestamp: RtmpTimestamp::ZERO,
        })
    }
}

/// play コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.2.1 節で定義される NetStream の play コマンドに対応する。
#[derive(Debug, Clone)]
pub struct RtmpPlayCommand {
    /// コマンドのトランザクション ID
    pub transaction_id: TransactionId,

    /// 再生するストリーム名
    pub stream_name: String,

    /// 再生を開始する位置（秒単位）
    ///
    /// RTMP 仕様 7.2.2.1 節では、省略時の既定値は `-2`。ライブを優先し、なければ録画を探し、
    /// どちらもなければライブ配信の開始を待つ。`-1` はライブのみ、`0` 以上は録画の開始位置を指定する。
    pub start: f64,
}

impl RtmpPlayCommand {
    fn from_message(
        transaction_id: TransactionId,
        _object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let stream_name = args
            .first()
            .ok_or_else(|| Error::invalid_data("play: missing stream name"))?
            .expect_str()?
            .to_string();
        // Adobe RTMP 仕様（2012 年 12 月版）7.2.2.1 節の既定値は、開始位置を省略した場合だけ適用する。
        // Start の省略時の規定が変わった場合は見直す。
        let start = match args.get(1) {
            Some(value) => value.expect_number()?,
            None => -2.0,
        };
        Ok(Self {
            transaction_id,
            stream_name,
            start,
        })
    }

    /// play コマンドを受け入れたことを示す _result メッセージを生成する
    ///
    /// 再生開始を表す `NetStream.Play.Start` を情報オブジェクトに含め、
    /// 指定されたストリームの ID のヘッダーで返す。
    pub fn accept(
        transaction_id: TransactionId,
        stream_id: RtmpMessageStreamId,
    ) -> Result<RtmpMessage, Error> {
        let properties = AmfValue::amf0_object(iter::empty());
        let information = AmfValue::amf0_object([
            ("level", Amf0Value::String("status".to_string())),
            (
                "code",
                Amf0Value::String("NetStream.Play.Start".to_string()),
            ),
            (
                "description",
                Amf0Value::String("Play succeeded.".to_string()),
            ),
        ]);
        let command = RtmpResultCommand {
            transaction_id,
            properties,
            information,
        };
        RtmpCommand::Result(command).into_message(RtmpMessageHeader {
            stream_id,
            timestamp: RtmpTimestamp::ZERO,
        })
    }
}

/// deleteStream コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.2.3 節で定義される NetStream の deleteStream コマンドに対応する。
#[derive(Debug, Clone)]
pub struct RtmpDeleteStreamCommand {
    /// コマンドのトランザクション ID
    pub transaction_id: TransactionId,

    /// 削除するストリームのメッセージストリーム ID
    pub stream_id: RtmpMessageStreamId,
}

impl RtmpDeleteStreamCommand {
    fn from_message(
        transaction_id: TransactionId,
        _object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let stream_id = args
            .first()
            .ok_or_else(|| Error::invalid_data("deleteStream: missing stream id"))?
            .expect_number()?;
        Ok(Self {
            transaction_id,
            stream_id: RtmpMessageStreamId::new(stream_id as u32),
        })
    }
}

/// getStreamLength コマンドの内容を保持する構造体
///
/// ストリームの長さを問い合わせる NetStream の getStreamLength コマンドに対応する。
#[derive(Debug, Clone)]
pub struct RtmpGetStreamLengthCommand {
    /// コマンドのトランザクション ID
    pub transaction_id: TransactionId,

    /// 長さを取得するストリーム名
    pub stream_name: String,
}

impl RtmpGetStreamLengthCommand {
    fn from_message(
        transaction_id: TransactionId,
        _object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let stream_name = args
            .first()
            .ok_or_else(|| Error::invalid_data("getStreamLength: missing stream name"))?
            .expect_str()?
            .to_string();
        Ok(Self {
            transaction_id,
            stream_name,
        })
    }
}

/// _result コマンドの内容を保持する構造体
///
/// コマンドに対する応答を表す。RTMP 仕様 7.2 節では、正常応答は _result、
/// エラー応答は _error として定義されている。
#[derive(Debug, Clone)]
pub struct RtmpResultCommand {
    /// 応答対象のコマンドのトランザクション ID
    pub transaction_id: TransactionId,

    /// コマンドオブジェクトとして送信するプロパティ
    pub properties: AmfValue,

    /// コマンドの最初の引数として送信する情報
    pub information: AmfValue,
}

impl RtmpResultCommand {
    /// getStreamLength コマンドの応答を生成する
    ///
    /// ストリームの長さを表す数値を情報として持つ _result コマンドを生成する。
    pub fn get_stream_length_result(transaction_id: TransactionId, length: f64) -> Self {
        Self {
            transaction_id,
            properties: AmfValue::Amf0(Amf0Value::Null),
            information: AmfValue::Amf0(Amf0Value::Number(length)),
        }
    }

    fn from_message(
        transaction_id: TransactionId,
        object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let properties = object;
        let information = args
            .first()
            .cloned()
            .ok_or_else(|| Error::invalid_data("_result: missing information argument"))?;
        Ok(Self {
            transaction_id,
            properties,
            information,
        })
    }

    /// エラー応答かどうかを判定する
    ///
    /// コマンドオブジェクトの `code` に `error` が含まれる場合に true を返す。
    /// 応答の成否は `code` の内容からヒューリスティックに判定する。
    pub fn is_error(&self) -> bool {
        // ヒューリスティックでエラー応答かどうかを判定する
        self.properties
            .expect_object_member("code")
            .and_then(|code| code.expect_str())
            .map(|code| code.to_lowercase().contains("error"))
            .unwrap_or(false)
    }

    /// createStream コマンドの応答を生成する
    ///
    /// 作成されたストリームの ID を数値として情報に持つ _result コマンドを生成する。
    pub fn create_stream_result(
        transaction_id: TransactionId,
        stream_id: RtmpMessageStreamId,
    ) -> Self {
        Self {
            transaction_id,
            properties: AmfValue::Amf0(Amf0Value::Null),
            information: AmfValue::Amf0(Amf0Value::Number(stream_id.get() as f64)),
        }
    }
}

/// onStatus コマンドの内容を保持する構造体
///
/// RTMP 仕様 7.2.2 節で定義される NetStream の状態通知に対応する。
/// Enhanced RTMP 仕様では、NetConnection の再接続要求などの通知にも使用される。
#[derive(Debug, Clone)]
pub struct RtmpOnStatusCommand {
    /// 状態の深刻度（`warning`、`status`、`error` のいずれか）
    pub level: String,

    /// 状態を表すコード（例: `NetStream.Publish.Start`）
    pub code: String,

    /// 状態の説明
    pub description: Option<String>,

    /// 状態の詳細
    pub details: Option<String>,
}

impl RtmpOnStatusCommand {
    fn from_message(
        _transaction_id: TransactionId,
        _object: AmfValue,
        args: Vec<AmfValue>,
    ) -> Result<Self, Error> {
        let status_obj = args
            .first()
            .ok_or_else(|| Error::invalid_data("onStatus: missing status object"))?;

        let level = status_obj
            .expect_object_member("level")?
            .expect_str()?
            .to_string();
        let code = status_obj
            .expect_object_member("code")?
            .expect_str()?
            .to_string();

        let description = status_obj
            .expect_object_member("description")
            .ok()
            .and_then(|v| v.expect_str().ok())
            .map(|s| s.to_string());

        let details = status_obj
            .expect_object_member("details")
            .ok()
            .and_then(|v| v.expect_str().ok())
            .map(|s| s.to_string());

        Ok(Self {
            level,
            code,
            description,
            details,
        })
    }

    /// 配信開始を表す onStatus コマンドかどうかを判定する
    ///
    /// `code` が `NetStream.Publish.Start` と等しい場合に true を返す。
    pub fn is_publish_start(&self) -> bool {
        self.code == "NetStream.Publish.Start"
    }

    /// 再生開始を表す onStatus コマンドかどうかを判定する
    ///
    /// `code` が `NetStream.Play.Start` と等しい場合に true を返す。
    pub fn is_play_start(&self) -> bool {
        self.code == "NetStream.Play.Start"
    }

    /// 配信開始を表す onStatus コマンドを生成する
    ///
    /// 状態コードは `NetStream.Publish.Start` となる。
    pub fn publish_start() -> Self {
        Self {
            level: "status".to_string(),
            code: "NetStream.Publish.Start".to_string(),
            description: Some("Publish succeeded.".to_string()),
            details: None,
        }
    }

    /// 再生開始を表す onStatus コマンドを生成する
    ///
    /// 状態コードは `NetStream.Play.Start` となる。
    pub fn play_start() -> Self {
        Self {
            level: "status".to_string(),
            code: "NetStream.Play.Start".to_string(),
            description: Some("Play succeeded.".to_string()),
            details: None,
        }
    }

    /// 配信名が使用済みであることを通知する onStatus コマンドを生成する
    ///
    /// 状態の深刻度は `error`、状態コードは `NetStream.Publish.BadName` となり、
    /// 指定された理由が詳細に格納される。
    pub fn publish_bad_name(reason: &str) -> Self {
        Self {
            level: "error".to_string(),
            code: "NetStream.Publish.BadName".to_string(),
            description: Some("Stream name already in use.".to_string()),
            details: Some(reason.to_string()),
        }
    }

    /// 再生対象のストリームが見つからないことを通知する onStatus コマンドを生成する
    ///
    /// 状態の深刻度は `error`、状態コードは `NetStream.Play.StreamNotFound` となり、
    /// 指定された理由が詳細に格納される。
    pub fn play_stream_not_found(reason: &str) -> Self {
        Self {
            level: "error".to_string(),
            code: "NetStream.Play.StreamNotFound".to_string(),
            description: Some("Stream not found.".to_string()),
            details: Some(reason.to_string()),
        }
    }
}
