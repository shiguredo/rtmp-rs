# サーバーが C2 を先に受信しても S2 の残りを送信する

- Created: 2026-11-21
- Completed: 2026-11-21
- Branch: feature/fix-server-handshake-send-buffer
- Polished: 2026-11-21

## 目的

仕様上有効な順序で `C2` が届いても、サーバーが `S2` を送り切り、相互接続がハンドシェイクで止まらないようにする。

## 現状

[Adobe RTMP 仕様（2012 年 12 月版）§5.2.1](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=7) では、クライアントは `S1` を受信すれば `C2` を送信できる。`S2` の受信を待つのは、クライアントが `C2` 以外の後続データを送る場合である。

`RtmpServerConnection::feed_recv_buf` は `C2` の受信を完了すると、送信の進捗とは独立に `Connecting` に遷移する。このとき `RtmpServerConnection::send_buf` はチャンク用バッファに切り替わるが、`RtmpServerConnection::advance_send_buf` は `RtmpServerHandshake::is_send_complete` が真になるまでハンドシェイク用バッファを消費する。

### 再現条件

1. サーバーに `C0/C1` を渡し、`S0/S1` までを送信済みとして反映する。`S2` は未送信のままにする。
2. サーバーの `S1` から [同仕様 §5.2.4](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=8) の形式で `C2` を組み立てて渡す。この時点では RTMP チャンクを渡さない。
3. `send_buf()` は残っている `S2` の 1536 バイトを返すべきだが、現行実装では空になる。`S2` を先に 123 バイト送信した場合も、残り 1413 バイトを返さず空になる。

## 設計方針

- `RtmpServerConnection::send_buf` がバッファを選択する条件を `RtmpServerConnection::advance_send_buf` と同じ `RtmpServerHandshake::is_send_complete` にそろえる。`Connecting` に遷移しても `S2` が残っている間はハンドシェイク用バッファを返し、送信済みとして反映し終えてからチャンク用バッファへ切り替える。
- 受信側の状態遷移、公開 API、ハンドシェイクの内容、ACK の計数は変更しない。

## 完了条件

- `S2` が全量または一部残る時点で `C2` を受信しても、公開 API で `S2` の残りをバイト列として取得し、送信済みとして消費できる。
- `C2` が先に届く順序と、`S2` を送り切ってから `C2` を受信する順序の両方で、ハンドシェイク完了後に実際の `connect` チャンクを受信させ、応答チャンクを取得して `advance_send_buf()` で消費できることを確認する。ネットワークを使わない公開 API の回帰テストで検証する。

## 解決方法

- サーバーの `send_buf()` と `advance_send_buf()` でハンドシェイクの送信完了判定をそろえた。`C2` の受信後も `S2` が残っていればそのバイト列を返し、送信を反映し終えてからチャンク用バッファに切り替える。受信側の状態遷移、ハンドシェイクの内容、ACK の計数は変更していない。
- `S2` の未送信・部分送信・全量送信の各時点に `C2` が届く場合を公開 API の性質テストで検証した。`S2` の残りと、後続の `connect` への応答チャンクの取得・消費まで確認した。
