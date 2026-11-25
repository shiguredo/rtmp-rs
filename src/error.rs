//! RTMP ライブラリのエラー型を提供するモジュール

use alloc::string::String;
use core::fmt;
use core::panic::Location;

/// エラーの種類
///
/// [`crate::amf::AmfValue::decode`] は、渡されたスライスから値を読み取るためのバイト数が
/// 足りない場合、`kind` が [`Self::InsufficientBuffer`] のエラーを返すことがある。
/// 再試行する場合は、元の入力に続きのバイトを加えたスライスを渡す。
///
/// [`crate::rtmp_message_decoder::RtmpMessageDecoder::decode`] は、チャンクのデコードが
/// 入力不足で止まった場合は `Ok(None)` を返す。宣言長まで受信したメッセージの解析に失敗した
/// 場合は `Err` を返す。このとき `kind` が [`Self::InsufficientBuffer`] でも、
/// 当該ペイロードを追加受信で補うことはできない。
///
/// # 例
///
/// すべてのバリアントをワイルドカードなしで分岐できる。
///
/// ```
/// use shiguredo_rtmp::ErrorKind;
///
/// fn describe(kind: ErrorKind) -> &'static str {
///     match kind {
///         ErrorKind::InvalidInput => "Invalid input",
///         ErrorKind::InvalidData => "Invalid data",
///         ErrorKind::InvalidState => "Invalid state",
///         ErrorKind::Unsupported => "Unsupported operation or data format",
///         ErrorKind::InsufficientBuffer => "Not enough bytes to read",
///     }
/// }
///
/// let _ = describe(ErrorKind::InsufficientBuffer);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// 入力パラメーターの形式または構造が無効である
    InvalidInput,

    /// 入力データが無効または破損している
    InvalidData,

    /// 構造体などの内部状態が不正だったり、依頼された操作を実行可能な状態ではない
    InvalidState,

    /// 操作またはデータ形式がサポートされていない
    Unsupported,

    /// 入力から読み取るために必要なバイト数が足りない
    ///
    /// 追加入力で再開できるかどうかは、呼び出した API の契約による。
    InsufficientBuffer,
}

/// エラー型
pub struct Error {
    /// 発生したエラーの種類
    pub kind: ErrorKind,

    /// エラーが発生した理由
    pub reason: String,

    /// エラーが作成されたソースコードの場所
    pub location: &'static Location<'static>,
}

impl Error {
    /// [`Error`] インスタンスを生成する
    #[track_caller]
    pub fn new(kind: ErrorKind) -> Self {
        Self::with_reason(kind, String::new())
    }

    /// エラー理由つきで [`Error`] インスタンスを生成する
    #[track_caller]
    pub fn with_reason<T: Into<String>>(kind: ErrorKind, reason: T) -> Self {
        Self {
            kind,
            reason: reason.into(),
            location: Location::caller(),
        }
    }

    #[track_caller]
    pub(crate) fn invalid_data<T: Into<String>>(reason: T) -> Self {
        Self::with_reason(ErrorKind::InvalidData, reason)
    }

    #[track_caller]
    pub(crate) fn invalid_input<T: Into<String>>(reason: T) -> Self {
        Self::with_reason(ErrorKind::InvalidInput, reason)
    }

    #[track_caller]
    pub(crate) fn invalid_state<T: Into<String>>(reason: T) -> Self {
        Self::with_reason(ErrorKind::InvalidState, reason)
    }

    #[track_caller]
    pub(crate) fn unsupported<T: Into<String>>(reason: T) -> Self {
        Self::with_reason(ErrorKind::Unsupported, reason)
    }

    #[track_caller]
    pub(crate) fn insufficient_buffer() -> Self {
        Self::new(ErrorKind::InsufficientBuffer)
    }

    #[track_caller]
    pub(crate) fn check_buffer_size(required_size: usize, buf: &[u8]) -> Result<(), Self> {
        if buf.len() < required_size {
            Err(Self::insufficient_buffer())
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.reason)?;
        write!(f, " (at {}:{})", self.location.file(), self.location.line())?;
        Ok(())
    }
}

impl core::error::Error for Error {}
