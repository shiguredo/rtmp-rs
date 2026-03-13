use shiguredo_rtmp::{Error, ErrorKind};

/// 発生する可能性のあるエラーの種類を表現する列挙型
#[repr(C)]
#[derive(Debug)]
#[expect(non_camel_case_types)]
pub enum RtmpCError {
    /// エラーが発生しなかったことを示す
    RTMP_ERROR_OK = 0,

    /// 入力引数ないしパラメーターが無効である
    RTMP_ERROR_INVALID_INPUT,

    /// 入力データが破損しているか無効な形式である
    RTMP_ERROR_INVALID_DATA,

    /// 操作に対する内部状態が無効である
    RTMP_ERROR_INVALID_STATE,

    /// NULL ポインタが渡された
    RTMP_ERROR_NULL_POINTER,

    /// 操作またはデータ形式がサポートされていない
    RTMP_ERROR_UNSUPPORTED,

    /// 上記以外のエラーが発生した
    RTMP_ERROR_OTHER,
}

impl From<Error> for RtmpCError {
    fn from(e: Error) -> Self {
        match e.kind {
            ErrorKind::InvalidInput => Self::RTMP_ERROR_INVALID_INPUT,
            ErrorKind::InvalidData => Self::RTMP_ERROR_INVALID_DATA,
            ErrorKind::InvalidState => Self::RTMP_ERROR_INVALID_STATE,
            ErrorKind::Unsupported => Self::RTMP_ERROR_UNSUPPORTED,
            _ => Self::RTMP_ERROR_OTHER,
        }
    }
}
