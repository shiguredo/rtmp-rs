//! RTMP のチャンクで使用する ID・サイズ・ヘッダー形式と、チャンク分割対象のメッセージを提供するモジュール
//!
//! RTMP 仕様 5.3 節に基づくチャンク分割を扱うための型を定義している。

use alloc::vec::Vec;

use crate::rtmp_message::{RtmpMessageStreamId, RtmpMessageType};
use crate::rtmp_timestamp::RtmpTimestamp;

/// RTMP のチャンクストリーム ID
///
/// チャンクが属するストリームを識別する 2 から 65599 までの値で、
/// RTMP 仕様 5.3.1.1 節のチャンク基本ヘッダーで指定される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RtmpChunkStreamId(u32);

impl RtmpChunkStreamId {
    /// チャンクストリーム ID を作成する
    ///
    /// 有効な値の範囲は 2 から 65599 までで、範囲外の値を指定した場合は `None` を返す。
    /// 0 と 1 は RTMP 仕様 5.3.1.1 節で予約されている。
    pub fn new(id: u32) -> Option<Self> {
        (2..=65599).contains(&id).then_some(Self(id))
    }

    /// メッセージストリーム ID からチャンクストリーム ID を求める
    ///
    /// メッセージストリーム ID に 2 を加算した値を使用し、65599 を超える場合は 65599 に丸める。
    /// この対応付けはこのライブラリが採用している規則であり、RTMP 仕様で定められたものではない。
    pub fn from_message_stream_id(message_stream_id: RtmpMessageStreamId) -> Self {
        Self((message_stream_id.get() + 2).min(65599))
    }

    /// チャンクストリーム ID の値を取得する
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// チャンクのメッセージヘッダー形式（fmt フィールド）
///
/// RTMP 仕様 5.3.1.2 節では、チャンクのメッセージヘッダーに 4 種類の形式があり、
/// 形式ごとに含まれるフィールドが異なる。形式はチャンク基本ヘッダーの先頭 2 ビットで指定される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MessageHeaderFormat {
    /// 形式 0
    ///
    /// タイムスタンプ（絶対値）、メッセージ長、メッセージ型 ID、メッセージストリーム ID を含む。
    /// チャンクストリームごとの最初のチャンクでは、この形式を使用する必要がある。
    F0 = 0,

    /// 形式 1
    ///
    /// タイムスタンプ差分、メッセージ長、メッセージ型 ID を含む。
    F1,

    /// 形式 2
    ///
    /// タイムスタンプ差分のみを含む。
    F2,

    /// 形式 3
    ///
    /// メッセージヘッダーを含まず、直前のチャンクのメッセージストリーム ID・メッセージ長・
    /// メッセージ型 ID を引き継ぐ。拡張タイムスタンプを使用している場合は、
    /// 4 バイトの拡張タイムスタンプのみが続く。
    F3,
}

/// RTMP のチャンクサイズ
///
/// 1 チャンクで送受信できるメッセージペイロードの最大バイト数で、
/// RTMP 仕様 5.4.1 節の Set Chunk Size メッセージによって変更できる。
/// 既定値は 128 バイト。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RtmpChunkSize(usize);

impl RtmpChunkSize {
    /// この実装が扱う最小のチャンクサイズ（1 バイト）
    pub const MIN: Self = Self(1);

    /// この実装が扱う最大のチャンクサイズ（65536 バイト）
    pub const MAX: Self = Self(65536);

    /// チャンクサイズを作成する
    ///
    /// 有効な値の範囲は [`RtmpChunkSize::MIN`] から [`RtmpChunkSize::MAX`] までで、
    /// 範囲外の値を指定した場合は `None` を返す。
    pub fn new(size: usize) -> Option<Self> {
        let this = Self(size);
        (Self::MIN..=Self::MAX).contains(&this).then_some(this)
    }

    /// チャンクサイズを作成し、範囲外の値を有効な範囲に丸める
    ///
    /// [`RtmpChunkSize::MIN`] 未満の値は [`RtmpChunkSize::MIN`] に、
    /// [`RtmpChunkSize::MAX`] を超える値は [`RtmpChunkSize::MAX`] に丸める。
    pub fn saturating_new(size: usize) -> Self {
        Self(size).clamp(Self::MIN, Self::MAX)
    }

    /// チャンクサイズをバイト数として取得する
    pub const fn get(self) -> usize {
        self.0
    }
}

/// 既定のチャンクサイズ（RTMP 仕様 5.4.1 節で定められた 128 バイト）を返す
impl Default for RtmpChunkSize {
    fn default() -> Self {
        Self(128)
    }
}

/// RTMP のチャンク分割で送受信するメッセージ
///
/// デコーダーでは、チャンクストリームから分割されて届いたペイロードを結合し、
/// 1 メッセージ分が揃った時点でこの構造体が生成される。
/// エンコーダーでは、この構造体が保持するペイロード全体をチャンクサイズに従って分割する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpChunk {
    /// メッセージを運ぶチャンクストリームの ID
    pub chunk_stream_id: RtmpChunkStreamId,

    /// メッセージが属するメッセージストリームの ID
    pub message_stream_id: RtmpMessageStreamId,

    /// メッセージの種類
    pub message_type: RtmpMessageType,

    /// メッセージのタイムスタンプ（ミリ秒単位）
    pub timestamp: RtmpTimestamp,

    /// 分割前のメッセージペイロード全体
    pub payload: Vec<u8>,
}
