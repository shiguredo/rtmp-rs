//! RTMP コネクションの状態・イベント・オプションと、メッセージ送受信チャネルを提供するモジュール
//! ACK の形式とウィンドウは Adobe RTMP 仕様（2012 年 12 月版）5.4.3 節と 5.4.4 節を参照する。

use alloc::borrow::ToOwned;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use core::fmt;

use crate::bytes::Buf;
use crate::error::Error;
use crate::media::{AudioFrame, VideoFrame};
use crate::rtmp_chunk::{RtmpChunkSize, RtmpChunkStreamId};
use crate::rtmp_command::RtmpCommand;
use crate::rtmp_message::RtmpMessage;
use crate::rtmp_message_decoder::RtmpMessageDecoder;
use crate::rtmp_message_encoder::RtmpMessageEncoder;
#[cfg(doc)]
use crate::rtmp_server_connection::RtmpServerConnection;
use crate::rtmp_user_control_event::RtmpUserControlEvent;

/// RTMP 接続のオプション
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpConnectionOptions {
    /// チャンクサイズ（デフォルト: 4096）
    pub chunk_size: RtmpChunkSize,

    /// 確認応答ウィンドウサイズ（デフォルト: 5MB）
    pub ack_window_size: u32,
}

impl Default for RtmpConnectionOptions {
    fn default() -> Self {
        Self {
            chunk_size: RtmpChunkSize::saturating_new(4096),
            ack_window_size: 5_000_000, // およそ 5MB のデータを受信する度に ack を送信する
        }
    }
}

/// RTMP コネクションで発生するイベント
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtmpConnectionEvent {
    /// クライアントから配信コマンドを受信した
    PublishRequested {
        /// アプリケーション名
        app: String,

        /// 接続 URL
        tc_url: String,

        /// ストリーム名
        stream_name: String,
    },

    /// クライアントから再生リクエストを受信した
    PlayRequested {
        /// アプリケーション名
        app: String,

        /// 接続 URL
        tc_url: String,

        /// ストリーム名
        stream_name: String,
    },

    /// 音声フレームを受信した
    AudioReceived(AudioFrame),

    /// 映像フレームを受信した
    VideoReceived(VideoFrame),

    /// コネクションの状態が変更された
    StateChanged(RtmpConnectionState),

    /// 相手から切断された
    DisconnectedByPeer {
        /// 切断理由
        reason: String,
    },

    /// RTMP のコマンドが無視された（デバッグ用イベント）
    ///
    /// サーバーおよびクライアントコネクションは明示的なハンドリングが不要なコマンドを受信した場合には
    /// 無視をするので、その通知 （通常は利用者がこのイベントを明示的にハンドリングする必要はない）
    CommandIgnored {
        /// コマンド名
        name: String,

        /// 詳細情報
        detail: String,
    },

    /// RTMP のメッセージが無視された（デバッグ用イベント）
    ///
    /// サーバーおよびクライアントコネクションは明示的なハンドリングが不要なイベントを受信した場合には
    /// 無視をするので、その通知 （通常は利用者がこのイベントを明示的にハンドリングする必要はない）
    MessageIgnored {
        /// メッセージ名
        name: String,

        /// 詳細情報
        detail: String,
    },

    /// RTMP のユーザーコントロールイベントが無視された（デバッグ用イベント）
    ///
    /// サーバーおよびクライアントコネクションは明示的なハンドリングが不要なユーザーコントロールイベントを受信した場合には
    /// 無視をするので、その通知（通常は利用者がこのイベントを明示的にハンドリングする必要はない）
    UserControlEventIgnored {
        /// イベント名
        name: String,

        /// 詳細情報
        detail: String,
    },
}

impl RtmpConnectionEvent {
    pub(crate) fn command_ignored(command: &RtmpCommand) -> Self {
        Self::CommandIgnored {
            name: command.name().to_owned(),
            detail: format!("{command:?}"),
        }
    }

    pub(crate) fn message_ignored(message: &RtmpMessage) -> Self {
        Self::MessageIgnored {
            name: format!("{:?}", message.message_type()),
            detail: format!("{message:?}"),
        }
    }

    pub(crate) fn user_control_event_ignored(event: &RtmpUserControlEvent) -> Self {
        Self::UserControlEventIgnored {
            name: event.name().to_owned(),
            detail: format!("{event:?}"),
        }
    }
}

/// RTMP コネクションの状態
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RtmpConnectionState {
    /// ハンドシェイク中
    #[default]
    Handshaking,

    /// 接続中
    Connecting,

    /// 接続済み
    Connected,

    /// メディアストリーム作成済み
    MediaStreamCreated,

    /// 配信待機中
    ///
    /// [`RtmpServerConnection::accept()`] が呼ばれると `Publishing` 状態に遷移する
    PublishPending,

    /// 配信中
    ///
    /// [`RtmpServerConnection::accept()`] が呼ばれると `Playing` 状態に遷移する
    Publishing,

    /// 再生待機中
    PlayPending,

    /// 再生中
    Playing,

    /// 切断中
    ///
    /// この状態に遷移したら「利用側はその TCP 接続を切断すべき」であることを意味する
    Disconnecting,
}

impl RtmpConnectionState {
    #[track_caller]
    pub(crate) fn expect(self, expected: Self) -> Result<(), Error> {
        if self == expected {
            Ok(())
        } else {
            Err(Error::invalid_state(format!(
                "expected connection state {expected}, but current state is {self}"
            )))
        }
    }
}

impl fmt::Display for RtmpConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RtmpConnectionState::Handshaking => write!(f, "HANDSHAKING"),
            RtmpConnectionState::Connecting => write!(f, "CONNECTING"),
            RtmpConnectionState::Connected => write!(f, "CONNECTED"),
            RtmpConnectionState::MediaStreamCreated => write!(f, "MEDIA_STREAM_CREATED"),
            RtmpConnectionState::PublishPending => write!(f, "PUBLISH_PENDING"),
            RtmpConnectionState::Publishing => write!(f, "PUBLISHING"),
            RtmpConnectionState::PlayPending => write!(f, "PLAY_PENDING"),
            RtmpConnectionState::Playing => write!(f, "PLAYING"),
            RtmpConnectionState::Disconnecting => write!(f, "DISCONNECTING"),
        }
    }
}

/// RTMP のメッセージをチャンクに変換して送受信するチャネル
///
/// エンコーダーとデコーダーを内部に持ち、送信用バッファと受信済みメッセージのキューを
/// 管理する。Adobe RTMP 仕様（2012 年 12 月版）5.4.3 節の ACK と 5.4.4 節のウィンドウサイズに基づいて、
/// ACK の送信タイミングと、相手からの ACK が途絶えた場合の検出も行う。
#[derive(Debug, Default)]
pub struct RtmpMessageChannel {
    decoder: RtmpMessageDecoder,
    encoder: RtmpMessageEncoder,
    decoded_messages: VecDeque<RtmpMessage>,
    send_buf: Buf,

    // こちらが送る ACK は従来どおり、チャンクだけを 32 ビットで数える。
    peer_ack_window_size: u32,
    total_bytes_received: u32,
    last_ack_sent: u32,

    local_ack_window_size: u32,
    total_bytes_sent: u64,     // 送信済みのチャンクの累計
    handshake_bytes_sent: u64, // 0 はハンドシェイク込みの起点を使わないことを表す
    acked_without_handshake: AckProgress,
    acked_with_handshake: AckProgress,
}

#[derive(Debug, Default)]
struct AckProgress {
    confirmed: u64,
    // 送信完了より先に届いた ACK の位置を、64 バイト区間の先頭とビット集合で保持する。
    // 部分送信に必要な中間位置を残しながら、1 位置につき 1 要素ずつ確保するのを避ける。
    pending: BTreeMap<u64, u64>,
}

impl AckProgress {
    const BLOCK_SIZE: u64 = u64::BITS as u64;

    fn observe(&mut self, position: u64, total_bytes_sent: u64) {
        if position <= total_bytes_sent {
            self.confirmed = self.confirmed.max(position);
        } else {
            let offset = position % Self::BLOCK_SIZE;
            let block_start = position - offset;
            *self.pending.entry(block_start).or_default() |= 1 << offset;
        }
    }

    fn confirm_sent(&mut self, total_bytes_sent: u64) {
        while let Some(mut block) = self.pending.first_entry() {
            let block_start = *block.key();
            let Some(sent_offset) = total_bytes_sent.checked_sub(block_start) else {
                break;
            };
            let last_offset = Self::BLOCK_SIZE - 1;
            let sent_mask = u64::MAX >> (last_offset - sent_offset.min(last_offset));
            let ready = *block.get() & sent_mask;
            if ready != 0 {
                self.confirmed = self.confirmed.max(block_start + u64::from(ready.ilog2()));
                *block.get_mut() &= !ready;
            }
            if *block.get() != 0 {
                break;
            }
            block.remove();
        }
    }
}

impl RtmpMessageChannel {
    /// 受信したバイト列をデコーダーに渡し、デコード済みのメッセージを内部キューに追加する
    ///
    /// ACK を送信する必要がある場合は、その時点の累計受信バイト数を `Some` で返す。
    /// デコードに失敗した場合は [`Error`] を返す。
    pub fn feed_recv_buf(&mut self, buf: &[u8]) -> Result<Option<u32>, Error> {
        self.decoder.feed_buf(buf);
        self.total_bytes_received = self.total_bytes_received.wrapping_add(buf.len() as u32);

        while let Some(message) = self.decoder.decode()? {
            self.decoded_messages.push_back(message);
        }

        // ACK を送る必要があるなら送るように呼び出し元に指示する
        //
        // 念のために peer_ack_window_size の半分を超過したら送るようにする
        //
        // 仕様的には peer_ack_window_size 分だけ受信したら送れば十分そうだけど、
        // RTMP は仕様が不明瞭なところがあり、クライアントが厳密にそれに準拠している保証もないため、
        // 安全側に倒して、早めに送信するようにしている
        // （この部分のオーバーヘッドもほぼ無視できる範囲のため）
        let unacked_bytes = self.total_bytes_received.wrapping_sub(self.last_ack_sent);
        if unacked_bytes > self.peer_ack_window_size / 2 {
            self.last_ack_sent = self.total_bytes_received;
            Ok(Some(self.total_bytes_received))
        } else {
            Ok(None)
        }
    }

    /// 送信待ちのチャンクデータを取得する
    pub fn send_buf(&self) -> &[u8] {
        self.send_buf.get()
    }

    /// 送信済みの `n` バイトを送信バッファから消費する
    ///
    /// 相手からの ACK が想定よりも遅れている場合は `false` を返す。
    /// `false` が返された場合、呼び出し側は送信を止めるか接続を切断するなどの対処をする必要がある。
    pub fn advance_send_buf(&mut self, n: usize) -> bool {
        self.total_bytes_sent = self.total_bytes_sent.saturating_add(n as u64);
        self.send_buf.advance(n);

        self.acked_without_handshake
            .confirm_sent(self.total_bytes_sent);
        self.acked_with_handshake
            .confirm_sent(self.total_bytes_sent);

        // 起点は決め打ちせず、どちらの解釈でも ACK が遅れている場合だけ切断判定に進む。
        let acked_bytes = self
            .acked_without_handshake
            .confirmed
            .max(self.acked_with_handshake.confirmed);
        let unacked_bytes = self.total_bytes_sent - acked_bytes;
        unacked_bytes <= u64::from(self.local_ack_window_size) * 2
    }

    /// デコード済みのメッセージを 1 つ取り出す
    ///
    /// キューが空の場合は `None` を返す。
    pub fn next_recv_message(&mut self) -> Option<RtmpMessage> {
        self.decoded_messages.pop_front()
    }

    /// メッセージをチャンクにエンコードして送信バッファの末尾に追加する
    ///
    /// メッセージのストリーム ID に対応するチャンクストリーム ID が使用される。
    ///
    /// # エラー
    ///
    /// [`RtmpMessageEncoder::encode`] が入力を拒否した場合は、そのエラーを返す。
    /// 送信バッファと後続のメッセージのチャンクエンコード状態は変わらない。
    pub fn feed_send_message(&mut self, message: RtmpMessage) -> Result<(), Error> {
        let chunk_stream_id = RtmpChunkStreamId::from_message_stream_id(message.header().stream_id);
        self.encoder
            .encode(self.send_buf.inner_mut(), chunk_stream_id, message)
    }

    /// 自身の ACK ウィンドウサイズを設定する
    ///
    /// 送信時に、このサイズの 2 倍を超える未 ACK バイトが発生すると
    /// [`Self::advance_send_buf()`] が `false` を返す。
    pub fn set_local_ack_window_size(&mut self, size: u32) {
        self.local_ack_window_size = size;
    }

    /// 相手の ACK ウィンドウサイズを設定する
    ///
    /// 受信時に、このサイズの半分を超える未 ACK バイトが発生すると
    /// [`Self::feed_recv_buf()`] が ACK を送信するための値を返す。
    pub fn set_peer_ack_window_size(&mut self, size: u32) {
        self.peer_ack_window_size = size;
    }

    /// 接続側が送信済みのハンドシェイク長を登録し、ハンドシェイク込みの起点を有効にする。
    pub(crate) fn set_handshake_bytes_sent(&mut self, size: u64) {
        self.handshake_bytes_sent = size;
    }

    fn last_confirmed_ack(&self) -> u64 {
        // 保留だけの ACK で復元の基準を進めると、後続の送信済み位置への ACK を取りこぼす。
        let with_handshake = self.acked_with_handshake.confirmed;
        if with_handshake == 0 {
            self.acked_without_handshake.confirmed
        } else {
            self.acked_without_handshake
                .confirmed
                .max(with_handshake + self.handshake_bytes_sent)
        }
    }

    /// 相手から受信した ACK の累計受信バイト数を記録する
    ///
    /// `acked_bytes` は相手が通知してきた 32 ビットの累計受信バイト数。
    /// 送信バッファにある位置を指す ACK が先に届いた場合は、
    /// [`Self::advance_send_buf()`] で実際の送信量が反映されるまで、確認済みには数えない。
    pub fn notify_ack_received(&mut self, acked_bytes: u32) {
        // この時点で送信バッファにもないデータの ACK は保留しない。
        let queued_end = self
            .total_bytes_sent
            .saturating_add(self.send_buf().len() as u64);
        let sent_upper = self
            .total_bytes_sent
            .saturating_add(self.handshake_bytes_sent);
        let queued_upper = queued_end.saturating_add(self.handshake_bytes_sent);
        let Some(ack) = restore_ack_position(
            acked_bytes,
            self.last_confirmed_ack(),
            sent_upper,
            queued_upper,
        ) else {
            return;
        };

        if ack <= queued_end {
            self.acked_without_handshake
                .observe(ack, self.total_bytes_sent);
        }
        if self.handshake_bytes_sent != 0 && ack > self.handshake_bytes_sent {
            self.acked_with_handshake
                .observe(ack - self.handshake_bytes_sent, self.total_bytes_sent);
        }
    }
}

fn restore_ack_position(
    acked_bytes: u32,
    last_confirmed_ack: u64,
    sent_upper: u64,
    queued_upper: u64,
) -> Option<u64> {
    // Adobe RTMP 仕様（2012 年 12 月版）5.4.3 節は累計値を 32 ビットとするが、
    // 計数起点と折り返しは明記していない。以下は実装間の互換性のための扱いであり、
    // 仕様でこれらが明確化された場合は見直す必要がある。
    const ACK_PERIOD: u64 = 1 << 32;
    let delta = u64::from(acked_bytes.wrapping_sub(last_confirmed_ack as u32));
    // 前回と同じ値は進捗がないか、1 周以上経過したかを区別できない。
    if delta == 0 {
        return None;
    }
    let ack = last_confirmed_ack.checked_add(delta)?;
    // 送信済みの範囲に候補があるなら、未送信の範囲に同じ値があっても考慮しない。
    let upper = if ack <= sent_upper {
        sent_upper
    } else {
        queued_upper
    };
    (ack <= upper && upper - ack < ACK_PERIOD).then_some(ack)
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::vec::Vec;

    use crate::media::{
        AudioFormat, AudioFrame, AudioSampleRate, VideoCodec, VideoFrame, VideoFrameType,
    };
    use crate::rtmp_message::{RtmpMessageHeader, RtmpMessageStreamId};
    use crate::rtmp_message_decoder::RtmpMessageDecoder;
    use crate::rtmp_timestamp::RtmpTimestamp;
    use crate::rtmp_timestamp::RtmpTimestampDelta;

    use crate::rtmp_handshake::HANDSHAKE_BYTES;

    #[test]
    fn handshake_ack_after_ack_with_ambiguous_origin() {
        let mut channel = RtmpMessageChannel {
            total_bytes_sent: 5_004_000,
            ..Default::default()
        };
        channel.set_local_ack_window_size(5_000_000);
        channel.set_handshake_bytes_sent(HANDSHAKE_BYTES);

        // 最初の ACK は両起点で送信済みの範囲に収まる。
        channel.notify_ack_received(5_003_073);
        assert_eq!(channel.acked_without_handshake.confirmed, 5_003_073);
        assert_eq!(channel.acked_with_handshake.confirmed, 5_000_000);

        // 最初の ACK で起点を固定せず、次はハンドシェイク込みでしか成立しない値を受理する。
        channel.total_bytes_sent = 10_000_000;
        channel.notify_ack_received(10_003_073);
        assert_eq!(channel.acked_without_handshake.confirmed, 5_003_073);
        assert_eq!(channel.acked_with_handshake.confirmed, 10_000_000);
        channel
            .feed_send_message(RtmpMessage::ack(0))
            .expect("ACK のエンコードは失敗しないはず");
        assert!(channel.advance_send_buf(1), "正常な ACK で切断しない");

        // 送信が進んでチャンク起点の範囲に入っても、直前と同じ ACK では更新しない。
        channel.total_bytes_sent = 10_003_073;
        channel.notify_ack_received(10_003_073);
        assert_eq!(channel.acked_without_handshake.confirmed, 5_003_073);
        assert_eq!(channel.acked_with_handshake.confirmed, 10_000_000);

        // 片方が閾値を超えても、もう一方がちょうど閾値なら送信を続ける。
        channel.total_bytes_sent = 19_999_999;
        assert!(channel.advance_send_buf(1), "片方が閾値以内なら切断しない");
        assert!(!channel.advance_send_buf(1), "両方が閾値を超えたら検出する");
    }

    #[test]
    fn acknowledgement_advances_across_wrap_for_both_origins() {
        const ACK_PERIOD: u64 = 1 << 32;
        for offset in [0, HANDSHAKE_BYTES] {
            let mut channel = RtmpMessageChannel {
                total_bytes_sent: 1_000_000,
                ..Default::default()
            };
            channel.set_handshake_bytes_sent(HANDSHAKE_BYTES);
            let first_ack = (channel.total_bytes_sent + offset) as u32;
            channel.notify_ack_received(first_ack);

            // ACK の値が 32 ビットで折り返す直前まで進める。
            channel.total_bytes_sent = ACK_PERIOD - 100 - offset;
            channel.notify_ack_received((ACK_PERIOD - 100) as u32);
            assert_eq!(channel.last_confirmed_ack(), ACK_PERIOD - 100);

            // 生の値は最初の ACK と同じだが、前回の位置以降では新しい周回にしか対応しない。
            channel.total_bytes_sent = ACK_PERIOD + 1_000_000;
            channel.notify_ack_received(first_ack);
            assert_eq!(
                channel.last_confirmed_ack(),
                ACK_PERIOD + u64::from(first_ack)
            );
            let acked_bytes = if offset == 0 {
                channel.acked_without_handshake.confirmed
            } else {
                channel.acked_with_handshake.confirmed
            };
            assert_eq!(acked_bytes, channel.total_bytes_sent);
        }
    }

    #[test]
    fn acknowledgement_does_not_advance_outside_sent_range() {
        let mut channel = RtmpMessageChannel {
            total_bytes_sent: 5_000_000,
            ..Default::default()
        };

        // 接続を介さないチャネルでは、ハンドシェイクによる先行を許容しない。
        channel.notify_ack_received(5_003_073);
        assert_eq!(channel.last_confirmed_ack(), 0);

        channel.set_handshake_bytes_sent(HANDSHAKE_BYTES);
        channel.notify_ack_received(5_003_073);
        // 許容できる最大の位置より 1 バイト先の ACK と、前回より後退した ACK は無効。
        for ack in [5_003_074, 5_003_072] {
            channel.notify_ack_received(ack);
            assert_eq!(channel.last_confirmed_ack(), 5_003_073);
            assert_eq!(channel.acked_without_handshake.confirmed, 0);
            assert_eq!(channel.acked_with_handshake.confirmed, 5_000_000);
        }
    }

    #[test]
    fn early_acknowledgements_wait_for_partial_send_notifications() {
        for offset in [0, HANDSHAKE_BYTES] {
            let mut channel = RtmpMessageChannel::default();
            channel.set_local_ack_window_size(1);
            channel.set_handshake_bytes_sent(offset);
            channel
                .feed_send_message(RtmpMessage::ack(0))
                .expect("ACK のエンコードは失敗しないはず");
            let queued = channel.send_buf().len() as u32;
            assert!(queued > 11, "閾値の検証に必要な送信データがあるはず");

            // 受信時点で送信バッファにもなかった位置は、後からバッファが増えても確認に使わない。
            channel.notify_ack_received(queued + offset as u32 + 1);
            assert!(channel.acked_without_handshake.pending.is_empty());
            assert!(channel.acked_with_handshake.pending.is_empty());

            // 二つの ACK が送信完了の通知より先に処理されても、送信を反映するまでは確認済みにしない。
            channel.notify_ack_received(4 + offset as u32);
            channel.notify_ack_received(8 + offset as u32);
            assert_eq!(channel.acked_without_handshake.confirmed, 0);
            assert_eq!(channel.acked_with_handshake.confirmed, 0);

            // 部分送信するたびに、その範囲に入った ACK だけを切断判定に反映する。
            let confirmed = |channel: &RtmpMessageChannel| {
                if offset == 0 {
                    channel.acked_without_handshake.confirmed
                } else {
                    channel.acked_with_handshake.confirmed
                }
            };
            assert!(channel.advance_send_buf(6));
            assert_eq!(confirmed(&channel), 4);
            assert!(channel.advance_send_buf(4));
            assert_eq!(confirmed(&channel), 8);
            assert!(!channel.advance_send_buf(1));
        }
    }

    #[test]
    fn pending_acknowledgement_does_not_block_confirmable_progress() {
        for offset in [0, HANDSHAKE_BYTES] {
            let mut channel = RtmpMessageChannel::default();
            channel.set_local_ack_window_size(3);
            channel.set_handshake_bytes_sent(offset);
            channel
                .feed_send_message(RtmpMessage::ack(0))
                .expect("ACK のエンコードは失敗しないはず");
            assert!(channel.advance_send_buf(6));

            // 未送信の位置を指す ACK は保留だけとし、後続の有効な位置を拒否する基準にしない。
            channel.notify_ack_received(14 + offset as u32);
            assert_eq!(channel.last_confirmed_ack(), 0);
            channel.notify_ack_received(6 + offset as u32);
            assert_eq!(channel.last_confirmed_ack(), 6 + offset);
            assert!(channel.advance_send_buf(6));
            assert!(!channel.advance_send_buf(1));
        }
    }

    #[test]
    fn pending_acknowledgements_keep_partial_progress_across_blocks() {
        let mut progress = AckProgress::default();
        // 区間の境界（最上位ビットと次の区間の最下位ビット）を部分送信でまたぐ。
        for position in [63, 64, 65, 127, 128] {
            progress.observe(position, 0);
        }
        for (sent, expected) in [(62, 0), (63, 63), (64, 64), (126, 65), (128, 128)] {
            progress.confirm_sent(sent);
            assert_eq!(
                progress.confirmed, expected,
                "送信済みの ACK だけを反映する"
            );
        }
        assert!(progress.pending.is_empty(), "確定した位置を保持し続けない");
    }

    #[test]
    fn acknowledgement_prefers_a_unique_sent_position_to_a_queued_wrap() {
        const ACK_PERIOD: u64 = 1 << 32;
        // 次の周回がキューにあるだけなら、送信済みの一意な位置を優先する。
        assert_eq!(
            restore_ack_position(101, 100, ACK_PERIOD + 100, ACK_PERIOD + 101),
            Some(101)
        );
        // 既に送信済みの範囲にも複数の周回がある場合は、一意に復元できない。
        assert_eq!(
            restore_ack_position(101, 100, ACK_PERIOD + 101, ACK_PERIOD + 101),
            None
        );
        // 送信済みの範囲に候補がなく、送信待ちの範囲でしか成立しない値は保留用に返す。
        assert_eq!(restore_ack_position(101, 100, 100, 101), Some(101));
        assert_eq!(restore_ack_position(101, 100, 100, ACK_PERIOD + 101), None);
        // 累計自体の表現範囲を超えるときに小さな値へ戻さない。
        assert_eq!(restore_ack_position(0, u64::MAX, u64::MAX, u64::MAX), None);
    }

    #[test]
    fn acknowledgement_ignores_ambiguous_wrap_with_large_window() {
        const ACK_PERIOD: u64 = 1 << 32;
        let mut channel = RtmpMessageChannel {
            total_bytes_sent: ACK_PERIOD + 20,
            ..Default::default()
        };
        channel.set_local_ack_window_size(u32::MAX);

        // 10 と 2^32 + 10 はどちらも送信済みなので、どちらを確認したか決められない。
        channel.notify_ack_received(10);
        assert_eq!(channel.last_confirmed_ack(), 0);
        assert_eq!(channel.acked_without_handshake.confirmed, 0);

        // 大きなウィンドウでも閾値の計算があふれず、閾値より 1 バイト超えたときだけ失敗する。
        channel.total_bytes_sent = u64::from(u32::MAX) * 2 - 1;
        channel
            .feed_send_message(RtmpMessage::ack(0))
            .expect("ACK のエンコードは失敗しないはず");
        assert!(channel.advance_send_buf(1), "閾値と同じ送信量は許容する");
        assert!(!channel.advance_send_buf(1), "閾値を超えた送信量は検出する");
    }

    #[test]
    fn message_channel_send_roundtrip_regression_pbt() {
        // PBT で観測した最新の最小ケースを単体テスト化
        let messages = vec![
            RtmpMessage::SetChunkSize {
                header: RtmpMessageHeader {
                    stream_id: RtmpMessageStreamId::PCM,
                    timestamp: RtmpTimestamp::ZERO,
                },
                size: RtmpChunkSize::new(1).expect("チャンクサイズは有効な値であるはず"),
            },
            RtmpMessage::Video {
                header: RtmpMessageHeader {
                    stream_id: RtmpMessageStreamId::new(65_597),
                    timestamp: RtmpTimestamp::from_millis(1),
                },
                frame: VideoFrame {
                    timestamp: RtmpTimestamp::from_millis(1),
                    composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                    frame_type: VideoFrameType::KeyFrame,
                    codec: VideoCodec::Jpeg,
                    avc_packet_type: None,
                    data: vec![],
                },
            },
            RtmpMessage::Audio {
                header: RtmpMessageHeader {
                    stream_id: RtmpMessageStreamId::new(65_598),
                    timestamp: RtmpTimestamp::from_millis(16_777_215),
                },
                frame: AudioFrame {
                    timestamp: RtmpTimestamp::from_millis(16_777_215),
                    format: AudioFormat::Mp3,
                    sample_rate: AudioSampleRate::Khz5,
                    is_8bit_sample: false,
                    is_stereo: false,
                    is_aac_sequence_header: false,
                    data: vec![0],
                },
            },
        ];

        let mut channel = RtmpMessageChannel::default();
        for message in &messages {
            channel
                .feed_send_message(message.clone())
                .expect("有効なメッセージのエンコードは失敗しないはず");
        }

        let buf = channel.send_buf().to_vec();
        let mut decoder = RtmpMessageDecoder::default();
        decoder.feed_buf(&buf);

        let mut decoded = Vec::new();
        while let Some(message) = decoder
            .decode()
            .expect("エンコード済みメッセージのデコードは失敗しないはず")
        {
            decoded.push(message);
        }

        assert_eq!(decoded, messages);
    }
}
