//! ハンドシェイクを含む ACK による誤切断を接続の公開 API で検証する。

use shiguredo_rtmp::error::Error;
use shiguredo_rtmp::rtmp_client_connection::RtmpPublishClientConnection;
use shiguredo_rtmp::rtmp_connection::{RtmpConnectionState, RtmpMessageChannel};
use shiguredo_rtmp::rtmp_message::RtmpMessage;
use shiguredo_rtmp::rtmp_server_connection::RtmpServerConnection;
use shiguredo_rtmp::rtmp_url::RtmpUrl;

const ACK_WINDOW: u32 = 1;
const NEXT_WRITE_SIZE: usize = 2;

#[test]
fn publish_client_accepts_handshake_inclusive_ack() {
    let (mut client, mut server, client_handshake_len, _) = handshake();
    let sent = transfer(
        client.send_buf().to_vec(),
        |n| client.advance_send_buf(n),
        |buf| server.feed_recv_buf(buf),
    );

    let control = ack_and_reduce_window(client_handshake_len, sent);
    client
        .feed_recv_buf(control.send_buf())
        .expect("クライアントは ACK と帯域設定を受信できるはず");
    assert!(
        client.send_buf().len() >= NEXT_WRITE_SIZE,
        "送信待ちの帯域設定への応答があるはず"
    );
    client.advance_send_buf(NEXT_WRITE_SIZE);
    assert_eq!(
        client.state(),
        RtmpConnectionState::Connecting,
        "ハンドシェイク込みの ACK でクライアントが切断されないはず"
    );
}

#[test]
fn server_accepts_handshake_inclusive_ack_after_receiving_c2() {
    let (mut client, mut server, _, server_handshake_len) = handshake();
    transfer(
        client.send_buf().to_vec(),
        |n| client.advance_send_buf(n),
        |buf| server.feed_recv_buf(buf),
    );
    let sent = transfer(
        server.send_buf().to_vec(),
        |n| server.advance_send_buf(n),
        |buf| client.feed_recv_buf(buf),
    );

    let control = ack_and_reduce_window(server_handshake_len, sent);
    server
        .feed_recv_buf(control.send_buf())
        .expect("サーバーは ACK と帯域設定を受信できるはず");
    assert!(
        server.send_buf().len() >= NEXT_WRITE_SIZE,
        "送信待ちの帯域設定への応答があるはず"
    );
    server.advance_send_buf(NEXT_WRITE_SIZE);
    assert_eq!(
        server.state(),
        RtmpConnectionState::Connected,
        "ハンドシェイク込みの ACK でサーバーが切断されないはず"
    );
}

#[test]
fn client_does_not_count_a_handshake_only_ack_as_chunk_progress() {
    // CONNECT のチャンクが十分に長い場合も、C2 の通知前に届いた ACK をそのチャンクに割り当てない。
    let url = RtmpUrl::parse(&format!(
        "rtmp://example.invalid/{}/stream",
        "a".repeat(3073)
    ))
    .expect("有効な RTMP URL のはず");
    let (mut client, mut server, c0_c1_len, _) = handshake_until_c2(url);
    let c2 = client.send_buf().to_vec();
    server
        .feed_recv_buf(&c2)
        .expect("サーバーが C2 を受信できるはず");
    let handshake_len = c0_c1_len + u32::try_from(c2.len()).expect("C2 の長さは u32 に収まるはず");

    // 実際に送ったのはハンドシェイクだけ。C2 の送信通知より先にその ACK が届く。
    let mut control = RtmpMessageChannel::default();
    control.feed_send_message(RtmpMessage::ack(handshake_len));
    control.feed_send_message(RtmpMessage::set_peer_bandwidth(ACK_WINDOW));
    client
        .feed_recv_buf(control.send_buf())
        .expect("クライアントが制御メッセージを受信できるはず");
    client.advance_send_buf(c2.len());
    assert!(client.send_buf().len() >= handshake_len as usize);
    client.advance_send_buf(handshake_len as usize);
    assert_eq!(
        client.state(),
        RtmpConnectionState::Disconnecting,
        "チャンクの ACK がないため閾値超過を検出するはず"
    );
}

fn handshake() -> (RtmpPublishClientConnection, RtmpServerConnection, u32, u32) {
    let url = RtmpUrl::parse("rtmp://example.invalid/live/stream").expect("有効な RTMP URL のはず");
    let (mut client, mut server, c0_c1_len, server_handshake_len) = handshake_until_c2(url);
    let c2_len = transfer(
        client.send_buf().to_vec(),
        |n| client.advance_send_buf(n),
        |buf| server.feed_recv_buf(buf),
    );
    assert_eq!(
        server.state(),
        RtmpConnectionState::Connecting,
        "C2 を受信するとサーバーのハンドシェイクが完了するはず"
    );
    (client, server, c0_c1_len + c2_len, server_handshake_len)
}

fn handshake_until_c2(
    url: RtmpUrl,
) -> (RtmpPublishClientConnection, RtmpServerConnection, u32, u32) {
    let mut client = RtmpPublishClientConnection::new(url);
    let mut server = RtmpServerConnection::new();
    let c0_c1_len = transfer(
        client.send_buf().to_vec(),
        |n| client.advance_send_buf(n),
        |buf| server.feed_recv_buf(buf),
    );
    let server_handshake_len = transfer(
        server.send_buf().to_vec(),
        |n| server.advance_send_buf(n),
        |buf| client.feed_recv_buf(buf),
    );
    // S0/S1/S2 の送信を先に完了させる。サーバーが完了へ遷移するのは、この後 C2 を受信したとき。
    assert_eq!(
        server.state(),
        RtmpConnectionState::Handshaking,
        "C2 を受信するまではサーバーのハンドシェイクが完了しないはず"
    );
    (client, server, c0_c1_len, server_handshake_len)
}

fn transfer(
    data: Vec<u8>,
    mut advance: impl FnMut(usize),
    mut receive: impl FnMut(&[u8]) -> Result<(), Error>,
) -> u32 {
    // 大きいハンドシェイクパケットも分割し、実際に渡したバイト数だけ送信済みとする。
    for part in data.chunks(512) {
        advance(part.len());
        receive(part).expect("接続は転送した RTMP データを受信できるはず");
    }
    u32::try_from(data.len()).expect("転送長は u32 に収まるはず")
}

fn ack_and_reduce_window(handshake_len: u32, sent: u32) -> RtmpMessageChannel {
    assert_eq!(handshake_len, 3073, "片方向のハンドシェイク長が正しいはず");
    assert!(sent > ACK_WINDOW * 2, "ACK が無効なら閾値を超えるはず");

    // 送信済みの全チャンクとハンドシェイクの合計を ACK に入れ、次の送信分の閾値を 2 バイトにする。
    // ハンドシェイク長を正しく差し引けば、次の 2 バイトはちょうど閾値で収まる。
    let mut control = RtmpMessageChannel::default();
    control.feed_send_message(RtmpMessage::ack(handshake_len + sent));
    control.feed_send_message(RtmpMessage::set_peer_bandwidth(ACK_WINDOW));
    control
}
