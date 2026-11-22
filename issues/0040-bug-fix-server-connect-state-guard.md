# サーバーが Publishing / Playing 中に connect を再受理しないようにする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-server-connect-state-guard
- Polished: {YYYY-MM-DD}

## 目的

サーバー接続の状態機械を守り、配信中・再生中に不正な connect コマンドを受けて状態が巻き戻らないようにする。

## 現状

- `src/rtmp_server_connection.rs` の `RtmpServerConnection::handle_connect_command` には状態検査がない。
- connect コマンドを受けると、現在の状態に関係なく `Connected` に遷移し、`connect_command` を上書きする。
- そのため、Publishing / Playing 中に connect を再送すると状態が `Connected` に戻り、以後の `send_audio` / `send_video` が `InvalidState` になる。

### 再現条件

1. `RtmpServerConnection` で connect → createStream → publish → `accept()` の順に処理し、状態を `Publishing` にする。
2. 同じ接続に connect コマンドをもう一度渡す。
3. 状態を確認する。

期待は `Publishing` の維持、またはエラー。実際は状態が `Connected` になる。

## 設計方針

- `handle_connect_command` の先頭で `self.state.expect(RtmpConnectionState::Connecting)` など、connect を受け付けてよい状態を検証する。
- connect の二重送信は `ErrorKind::InvalidState` として扱い、切断状態に遷移させるか、利用側にエラーを返す。
- 既存の正常な connect の処理とイベント順序に回帰がないようにする。
- クライアント側の connect 処理にも同様の状態検査が必要か確認する。

## 完了条件

- Publishing / Playing 中の connect が受理されず、状態が巻き戻らない。
- 正常な connect の処理に回帰がない。
- connect の二重送信を確認する公開 API の回帰テストを追加する。

## 解決方法

1. `RtmpServerConnection::handle_connect_command` に状態検査を追加する。
2. 不正な状態での connect を `ErrorKind::InvalidState` として返す。
3. 二重 connect の回帰テストを追加する。
