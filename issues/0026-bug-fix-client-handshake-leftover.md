# クライアントがハンドシェイクと同時に受信したメッセージを処理するようにする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-client-handshake-leftover
- Polished: {YYYY-MM-DD}

## 目的

ハンドシェイク完了と同一の受信バッファに含まれていたメッセージを、クライアントが同じ `feed_recv_buf` 呼び出しで処理するようにする。サーバーが Ping 応答を待ってから次のデータを送る場合でも、クライアントが停止しないようにする。

## 現状

- `src/rtmp_client_connection.rs` の `RtmpClientConnection::feed_recv_buf` は、ハンドシェイク完了時に `change_state(RtmpConnectionState::Connecting)` を呼び、
  `handshake.take_recv_buf()` で残りを取得して `message_channel.feed_recv_buf` に渡す。しかし、その後に `message_channel.next_recv_message()` を取り出して `handle_recv_message` に渡すループがない。
- サーバー側の `src/rtmp_server_connection.rs` の `RtmpServerConnection::feed_recv_buf` は、同じ位置でデコード済みメッセージの処理ループを実行している。クライアントとサーバーで挙動が非対称になっている。
- そのため、ハンドシェイクと同時に届いたメッセージはデコーダのキューに残り、次に `feed_recv_buf` が呼ばれるまで処理されない。

### 再現条件

1. `RtmpPublishClientConnection` または `RtmpPlayClientConnection` に、S0 / S1 / S2 の 3073 バイトと、その直後の PingRequest（UserControl、イベント型 6、タイムスタンプ 1000）のチャンクを 1 つのバッファとして `feed_recv_buf` で渡す。
2. C2 を送信済みにして送信バッファをメッセージチャネルへ切り替える。
3. `send_buf` を確認すると PingResponse が積まれていない。空の `feed_recv_buf(&[])` を呼ぶと、そこで初めて PingResponse が積まれる。

期待は 1 回目の `feed_recv_buf` で PingResponse が送信バッファに積まれることである。

## 設計方針

- サーバー実装と同じく、ハンドシェイク完了後に残りバッファを `message_channel.feed_recv_buf` に渡したら、`message_channel.next_recv_message()` のループを実行して `handle_recv_message` に渡す。
- `feed_recv_buf` の戻り値（ACK を送る必要がある場合の累計受信バイト数）も、通常の受信経路と同じように処理する。
- ハンドシェイク完了時の状態遷移（`Connecting`）とメッセージ処理の順序を明確にし、`connect` コマンドの送信より前に届いた制御メッセージを取りこぼさない。
- 既存の公開 API は変更しない。

## 完了条件

- ハンドシェイクと同じ受信バッファに PingRequest を入れた場合、1 回目の `feed_recv_buf` で PingResponse が送信バッファに積まれる。
- ハンドシェイク完了直後に WinAckSize や SetPeerBandwidth が届いた場合も、同じ `feed_recv_buf` で設定が反映される。
- ハンドシェイク完了とメッセージ受信が別々のバッファで届く既存の経路に回帰がない。
- 上記を確認する公開 API の回帰テストを追加する。

## 解決方法

1. `RtmpClientConnection::feed_recv_buf` のハンドシェイク完了分岐で、残りバッファを `message_channel.feed_recv_buf` に渡した後にメッセージ処理ループを実行する。
2. `feed_recv_buf` の戻り値に応じた ACK 送信処理を共通化し、通常経路とハンドシェイク完了経路の両方から呼ぶ。
3. ハンドシェイク、PingRequest、WinAckSize を組み合わせた回帰テストを `tests/` に追加する。
