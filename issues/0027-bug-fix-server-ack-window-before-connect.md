# connect 前の Ping 応答送信でサーバーが自分から切断状態になる問題を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-server-ack-window-before-connect
- Polished: {YYYY-MM-DD}

## 目的

クライアントが connect コマンドより前に PingRequest を送っても、サーバーが自分の PingResponse の送信を理由に切断状態へ遷移しないようにする。

## 現状

- `src/rtmp_connection.rs` の `RtmpMessageChannel::default` は `local_ack_window_size` を 0 で初期化する。
- サーバーが `local_ack_window_size` を設定するのは `src/rtmp_server_connection.rs` の `RtmpServerConnection::handle_connect_command`（connect コマンドの処理時）だけである。
- connect より前に PingRequest を受信すると、`RtmpServerConnection::handle_recv_message` が PingResponse を送信バッファに積む。利用側が `RtmpServerConnection::advance_send_buf` で送信済みにすると、
  `RtmpMessageChannel::advance_send_buf` の判定 `total_bytes_sent - acked_bytes <= local_ack_window_size * 2` が `n > 0` で false になり、
  `DisconnectedByPeer` イベントと `Disconnecting` への遷移が発生する。
- RTMP 仕様ではハンドシェイク完了後に制御メッセージを送ることは有効であり、connect 前の Ping はプロトコル違反ではない。

### 再現条件

1. `RtmpServerConnection` に C0 / C1（1536 バイト）を渡す。
2. S0 / S1 / S2 を送信済みとして `advance_send_buf` を呼ぶ。
3. C2（1536 バイト）と PingRequest のチャンクを 1 つのバッファとして `feed_recv_buf` で渡す。
4. `send_buf` から PingResponse を取得し、その長さを `advance_send_buf` に渡す。

期待は状態が `Connecting` のままであること。実際は状態が `Disconnecting` になり、`DisconnectedByPeer` イベントが積まれる。

## 設計方針

- 接続開始時（ハンドシェイク完了時）に `RtmpConnectionOptions::ack_window_size` を `RtmpMessageChannel` のローカル ACK ウィンドウへ設定する。connect コマンドの受信を契機にしない。
- クライアント側も同様に、ハンドシェイク完了時にローカル ACK ウィンドウが設定されていることを確認する。
- connect 前にチャンクを送信する経路（Ping 応答、制御メッセージ）でも、ACK 途絶の判定が誤って発動しないようにする。
- 公開 API のシグネチャは変更しない。

## 完了条件

- 上記の再現条件で状態が `Connecting` のままになり、`DisconnectedByPeer` が積まれない。
- connect コマンドを通常どおり受信した後の ACK 途絶検出は従来どおり動作する。
- ローカル ACK ウィンドウの初期値が、connect 前後で一貫していることを確認する回帰テストを追加する。

## 解決方法

1. `RtmpServerConnection` のハンドシェイク完了時に `message_channel.set_local_ack_window_size(options.ack_window_size)` を呼ぶ。
2. `RtmpClientConnection` のハンドシェイク完了時も同様に設定する。
3. connect 前に Ping を送るシナリオと、connect 後に ACK が途絶えるシナリオの両方を公開 API のテストで確認する。
