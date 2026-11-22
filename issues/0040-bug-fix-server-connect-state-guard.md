# サーバーは Connecting のときだけ connect を受理する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-server-connect-state-guard
- Polished: 2026-11-22

## 目的

サーバーが初回接続のために `connect` を待っている場合だけ受理し、再受理によって接続状態や保存済みの接続情報が変わらないようにする。

## 現状

- `src/rtmp_server_connection.rs` の `RtmpServerConnection::handle_connect_command` には状態検査がない。
- ハンドラーは `connect` を処理すると接続用のメッセージを追加し、`Connected` に遷移して `connect_command` を上書きする。`Connecting` かどうかは確認しない。
- `Playing` から `Connected` に戻った場合、`Playing` を前提とするサーバーの `send_audio` / `send_video` が `InvalidState` になる。`Publishing` における問題は、サーバーの送信可否ではなく、配信中の状態と保存済みの接続情報が失われることにある。

### 再現条件

初回接続とストリーム作成を済ませたサーバーを対象とする。たとえば `Playing` で `connect` を受けた場合、期待値は `feed_recv_buf` が `ErrorKind::InvalidState` を返し、状態が `Playing` のままであること。現行コードには受理する状態を限定する分岐がなく、有効な `connect` として処理が完了すると `Connected` に変わる。

## 設計方針

- `RtmpServerConnection::handle_connect_command` の先頭で `self.state.expect(RtmpConnectionState::Connecting)?` により状態を検証する。`RtmpCommand::Connect` に変換できたコマンドは `Connecting` の場合だけ受理する。コマンドに変換するまでの解析エラーの扱いは変更しない。[Adobe RTMP 仕様 §7.2.1.1](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=29) は接続要求としての用途を定義しており、この状態制限は本ライブラリの契約とする。
- それ以外の状態では公開 `feed_recv_buf` に `ErrorKind::InvalidState` を返す。ハンドラーは保存済みの接続情報、サーバーの状態、`connect` に対する応答と状態遷移イベントを変更しない。ACK や同じ入力に先行するメッセージの処理は従来どおりとし、切断の判断は API の呼び出し元に委ねる。
- `Connecting` での正しい初回接続は、Window Acknowledgement Size、Set Chunk Size、Set Peer Bandwidth、Stream Begin、`_result` の順に送信バッファへ追加し、`StateChanged(Connected)` を従来どおり通知する。クライアントの接続成功応答には既に状態検査があり、本 issue ではクライアント側を変更しない。

## 完了条件

- 公開 API を利用するサーバーの回帰テストで、初回接続は受理され、`Connected`、`Publishing`、`Playing` では `connect` が `ErrorKind::InvalidState` になることを確認する。拒否時は元の状態を維持し、`connect` に由来する制御メッセージ、成功応答、状態遷移イベントを追加しない。`Connected` での拒否後に配信・再生リクエストを処理し、そのイベントの `app` と `tc_url` が初回接続の値のままであることも確認する。
- 既存の正常接続と配信・再生のテストが通り、初回接続の応答の順序と状態遷移イベントに回帰がない。既存のサーバー接続の PBT を利用し、エラー種別は文字列ではなく公開 `Error` の `kind` フィールドを確認する。テスト専用の内部 API は追加しない。

## 解決方法

1. `RtmpServerConnection::handle_connect_command` の副作用より前に状態検査を追加し、受理条件をコメントで明示する。
2. 呼び出し元の既存の `Result` 経路でエラーを伝播し、公開 API の回帰テストで拒否時の状態と応答を確認する。
