# deleteStream を受理して同じ接続で次のストリームを作成できるようにする

- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-server-delete-stream
- Polished: {YYYY-MM-DD}

## 目的

クライアントが `deleteStream` でストリームを破棄した後も、同じ接続で次のストリームを作成できるようにする。

## 現状

`rtmp_server_connection` の `RtmpServerConnection::handle_command` は、`connect`、`createStream`、`publish`、`play`、`getStreamLength` だけを処理し、それ以外は `RtmpConnectionEvent::command_ignored` として通知する。`deleteStream` もこの経路に入るため、ストリームを破棄しても状態は `MediaStreamCreated` のまま残る。

`createStream` は `RtmpConnectionState::Connected` のときだけ受理するため、`deleteStream` の後に同じ接続で `createStream` を送ると `InvalidState` になる。

[Adobe RTMP 仕様（2012 年 12 月版）§7.2.2.3](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=43) は、`deleteStream` を NetStream オブジェクトの破棄時に送るコマンドとし、ストリーム ID を引数に取ると定義している。サーバーは応答を返さない。

### 再現条件

1. サーバーのハンドシェイクを完了し、`connect` と `createStream` を処理して `MediaStreamCreated` にする。
2. `deleteStream` を、`createStream` で返したストリーム ID を引数にして送る。
3. 同じ接続で `createStream` を送る。

現行は 2 で `command_ignored` が通知されるだけで状態が変わらず、3 が `InvalidState` になる。期待は、2 でストリームの破棄が完了し、3 が受理されること。

## 設計方針

- `deleteStream` を `RtmpServerConnection` で処理する。引数のストリーム ID が現在のメディアストリームの ID と一致する場合だけ破棄し、状態を `Connected` に戻す。
- 破棄したことを利用側へ通知するイベントを追加する。公開 enum への追加は破壊的変更として扱い、`CHANGES.md` はこのリポジトリの方針どおり更新しない。
- ストリーム ID が一致しない場合や、ストリームを保持していない状態で受けた場合は、接続を切断せず `command_ignored` として扱う。
- サーバーは `deleteStream` への応答を返さない。`closeStream` や NetConnection の終了は対象外とする。
- 複数ストリームの同時利用は対象外とし、現在の 1 ストリーム前提を維持する。

## 完了条件

- 公開 API で、`createStream` の後に `deleteStream` を送るとストリームの破棄が通知され、状態が `Connected` になる。
- 続けて `createStream` を送ると受理され、`MediaStreamCreated` に戻る。
- 存在しないストリーム ID の `deleteStream` では状態が変わらず、接続エラーにならない。
- 上記を公開 API の性質テストで検証する。
