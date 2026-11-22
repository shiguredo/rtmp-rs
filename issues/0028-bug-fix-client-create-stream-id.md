# createStream の応答で返されたストリーム ID を使用する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-client-create-stream-id
- Polished: {YYYY-MM-DD}

## 目的

クライアントが createStream の `_result` でサーバーが返したメッセージストリーム ID を使用し、外部サーバーとの相互接続で publish / play が失敗しないようにする。

## 現状

- `src/rtmp_client_connection.rs` の `RtmpClientConnection::handle_result_command` は createStream の `_result` をトランザクション ID だけで判定し、`information`（サーバーが返したストリーム ID）を検証も保存もしない。
- publish / play / 音声 / 映像のメッセージは、常に `src/rtmp_message.rs` の `RtmpMessageStreamId::MEDIA`（値 2）で送信される。
- ライブラリ同梱のサーバーは常に `RtmpMessageStreamId::MEDIA` を返すため、ループバックのテストでは問題が露呈しない。

### 再現条件

1. クライアントのハンドシェイクを完了させ、connect の `_result`（トランザクション ID 1）を渡す。
2. createStream の `_result`（トランザクション ID 2）で、`information` の数値を 1 として返す。
3. クライアントが送信する publish メッセージのメッセージストリーム ID を確認する。

期待は 1。実際は 2 になる。

## 設計方針

- createStream の `_result` の `information` から数値を取り出し、接続内に保持する。
- publish / play と、その後の音声 / 映像メッセージで保持したストリーム ID を使用する。
- `information` が数値でない場合や `u32` の範囲外の場合は `ErrorKind::InvalidData` とする。
- `RtmpMessageStreamId::MEDIA` はサーバー側の既定値として残すが、クライアントはサーバーの応答を正とする。
- 公開 API に新しいフィールドを追加する場合は、既存の利用側が影響を受けない形にする。

## 完了条件

- createStream の応答で 1 が返された場合、publish / play とメディアメッセージのメッセージストリーム ID が 1 になる。
- 応答で 2 が返された場合は従来どおり 2 になる。
- 応答の `information` が数値でない場合は接続エラーになる。
- 上記を確認する公開 API の回帰テストを追加する。

## 解決方法

1. `RtmpClientConnection` に createStream で受け取ったストリーム ID を保持するフィールドを追加する。
2. `handle_result_command` の createStream 分岐で `information` を数値として取り出して保存する。
3. `RtmpPublishClientConnection::publish` / `RtmpPlayClientConnection::play`、および音声 / 映像送信で保持した ID を使う。
4. サーバーが 1 を返す場合の回帰テストを追加する。
