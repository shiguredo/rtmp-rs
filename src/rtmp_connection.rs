//! RTMP コネクションの状態・イベント・オプションと、メッセージ送受信チャネルを提供するモジュール
//! ACK の形式とウィンドウは Adobe RTMP 仕様（2012 年 12 月版）5.4.3 節と 5.4.4 節を参照する。

use alloc::borrow::ToOwned;
use alloc::collections::VecDeque;
use alloc::string::String;
use core::fmt;

use crate::bytes::Buf;
use crate::error::Error;
use crate::media::{AudioFrame, VideoFrame};
use crate::rtmp_chunk::{RtmpChunkSize, RtmpChunkStreamId};
use crate::rtmp_command::RtmpCommand;
use crate::rtmp_message::{RtmpMessage, RtmpMessageHeader, RtmpMessageType};
use crate::rtmp_message_decoder::{ReceivedMessage, RtmpMessageDecoder};
use crate::rtmp_message_encoder::RtmpMessageEncoder;
#[cfg(doc)]
use crate::rtmp_server_connection::RtmpServerConnection;
use crate::rtmp_user_control_event::RtmpUserControlEvent;

/// RTMP 接続のオプション
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtmpConnectionOptions {
    /// チャンクサイズ（デフォルト: 4096）
    pub chunk_size: RtmpChunkSize,

    /// 相手へ通知する確認応答ウィンドウサイズ（デフォルト: 5,000,000 バイト）
    ///
    /// Set Peer Bandwidth への応答では、この値を再通知する。相手が指定したサイズや
    /// 制限種別を受諾する応答ではない。このライブラリは受信 ACK に基づく送信量の制限を行わない。
    pub ack_window_size: u32,
}

impl Default for RtmpConnectionOptions {
    fn default() -> Self {
        Self {
            chunk_size: RtmpChunkSize::saturating_new(4096),
            ack_window_size: 5_000_000,
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

    /// 相手からの確認応答 (ACK) を処理した
    ///
    /// 重複や後退も含め、相手が通知した値を受信順に返す。自分の送信済みバイト数に換算せず、
    /// 送信可否や接続状態の判定には使わない。
    AckReceived {
        /// 通信上の 32 ビットのシーケンス番号
        sequence_number: u32,
    },

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
    /// 明示的な処理が不要なメッセージや、内容に対応していない Shared Object と Aggregate の通知。
    /// Shared Object と Aggregate の詳細にはメッセージストリーム ID とタイムスタンプだけを含める。
    /// 通常は利用者がこのイベントを明示的に処理する必要はない。
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

    pub(crate) fn unsupported_message_ignored(
        message_type: RtmpMessageType,
        header: RtmpMessageHeader,
    ) -> Self {
        Self::MessageIgnored {
            name: format!("{message_type:?}"),
            detail: format!(
                "message stream ID: {}, timestamp: {} ms",
                header.stream_id.get(),
                header.timestamp.as_millis()
            ),
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
/// 受信したデータに対する ACK の送信タイミングを判定する。相手からの ACK の値や有無では送信を制限しない。
#[derive(Debug, Default)]
pub struct RtmpMessageChannel {
    decoder: RtmpMessageDecoder,
    encoder: RtmpMessageEncoder,
    received_messages: VecDeque<ReceivedMessage>,
    send_buf: Buf,

    // こちらが送る ACK は、ハンドシェイクを含めずチャンクだけを 32 ビットで数える。
    peer_ack_window_size: u32,
    total_bytes_received: u32,
    last_ack_sent: u32,
}

impl RtmpMessageChannel {
    /// 受信したバイト列をデコーダーに渡し、組み立てが完了したメッセージを順に内部キューへ追加する
    ///
    /// ACK を送信する必要がある場合は、その時点の累計受信バイト数を `Some` で返す。
    /// デコードに失敗した場合は [`Error`] を返す。
    pub fn feed_recv_buf(&mut self, buf: &[u8]) -> Result<Option<u32>, Error> {
        self.decoder.feed_buf(buf);
        self.total_bytes_received = self.total_bytes_received.wrapping_add(buf.len() as u32);

        while let Some(message) = self.decoder.decode_next()? {
            self.received_messages.push_back(message);
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
    /// `n` は [`Self::send_buf()`] が返すバイト列の長さ以下でなければならない。
    pub fn advance_send_buf(&mut self, n: usize) {
        self.send_buf.advance(n);
    }

    /// 対応済みのメッセージを 1 つ取り出す
    ///
    /// Shared Object（AMF0 / AMF3）と Aggregate は読み飛ばす。
    /// 読み飛ばした後も対応済みのメッセージがあれば同じ呼び出しで返し、なければ `None` を返す。
    pub fn next_recv_message(&mut self) -> Option<RtmpMessage> {
        while let Some(received) = self.next_received() {
            if let ReceivedMessage::Message(message) = received {
                return Some(message);
            }
        }
        None
    }

    /// 内容を無視するメッセージも含めて、組み立てが完了した順に取り出す。
    pub(crate) fn next_received(&mut self) -> Option<ReceivedMessage> {
        self.received_messages.pop_front()
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

    /// 相手の ACK ウィンドウサイズを設定する
    ///
    /// 受信時に、このサイズの半分を超える未 ACK バイトが発生すると
    /// [`Self::feed_recv_buf()`] が ACK を送信するための値を返す。
    pub fn set_peer_ack_window_size(&mut self, size: u32) {
        self.peer_ack_window_size = size;
    }
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
