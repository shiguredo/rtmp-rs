# `_error` コマンドを解釈してエラー応答を検知する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-error-command-handling
- Polished: {YYYY-MM-DD}

## 目的

サーバーが `_error` コマンドで返した失敗をクライアントが検知し、接続状態を適切に遷移させるようにする。

## 現状

- `src/rtmp_command.rs` の `RtmpCommand::from_message` は `connect`、`createStream`、`publish`、`play`、`_result`、`onStatus` などを解釈するが、`_error` の分岐がなく `RtmpCommand::Ignore` になる。
- `src/rtmp_client_connection.rs` の `RtmpClientConnection::handle_command` は `RtmpCommand::Result` と `RtmpCommand::OnStatus` 以外を無視するため、connect の失敗でも状態が `Connecting` のまま、
  publish / play の失敗でも `PublishPending` / `PlayPending` のままになる。
- `RtmpResultCommand::is_error` は `properties`（コマンドオブジェクト）の `code` を見るが、connect のエラーコードは `information`（第 1 引数）に入る。`information` の `code` や `level` を見ていない。

### 再現条件

1. クライアントのハンドシェイクを完了させ、C2 を送信済みにする。
2. connect に対する `_error` コマンドを渡す。コマンド名は `_error`、トランザクション ID は 1、情報オブジェクトに `code = NetConnection.Connect.Failed` を持つ。
3. クライアントの状態と `next_event` を確認する。

期待は `Disconnecting` への遷移と `DisconnectedByPeer` イベント。実際は状態が `Connecting` のままで、切断イベントも出ない。

## 設計方針

- `_error` をコマンドとして解釈し、`_result` と同じトランザクション ID と情報オブジェクトを持つ型として扱う。公開 API の追加は `RtmpCommand` の variant 追加または既存の `RtmpResultCommand` の再利用のどちらかを選び、利用側の破壊的変更を最小化する。
- クライアントのコマンド処理で `_error` を切断扱いにし、理由に `information` の `code` / `description` を含める。
- `RtmpResultCommand::is_error` は `information` の `code` と `level` を見るようにする。`properties` を見る既存の判定は、`_result` の形によっては有効なので残すか、判定を統合するかを設計方針で明記する。
- サーバー側が `_error` を送る経路は本 issue の対象外とし、クライアントの受信処理に限定する。

## 完了条件

- connect の `_error` を受信すると `Disconnecting` に遷移し、`DisconnectedByPeer` が発行される。
- publish / play の `_error` を受信しても状態が `PublishPending` / `PlayPending` のままにならない。
- `_result` の `information.code` が `NetConnection.Connect.Failed` の場合も失敗として扱われる。
- 上記を確認する公開 API の回帰テストを追加する。

## 解決方法

1. `RtmpCommand::from_message` に `_error` の分岐を追加する。
2. `RtmpClientConnection::handle_command` で `_error` を処理し、`DisconnectedByPeer` と `Disconnecting` に遷移させる。
3. `RtmpResultCommand::is_error` を `information` の `code` / `level` も見るように修正する。
4. connect 失敗と publish 失敗の回帰テストを `tests/` に追加する。
