# Enhanced RTMP の Reconnect Request 対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-reconnect-request
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Reconnect Request 仕様 (`NetConnection.Connect.ReconnectRequest`) に対応し、サーバーがクライアントに再接続を促す `onStatus` を扱えるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリで、ライブ配信サーバーのアップデート時や、負荷分散・geolocation 最適化のためのクライアント再マッピングをサポートできるようになる。

## 優先度根拠

High。eRTMP v2 仕様で `CapsExMask.Reconnect` フラグでクライアント能力として明示的に宣言される機能であり、完全準拠を掲げる以上、対応必須。
ライブ配信サーバーのローリングアップデートや、Sora 等の SFU 接続先の再マッピングといった運用シナリオで効果的に機能する。

## 現状

- `RtmpServerConnection` / `RtmpPublishClientConnection` / `RtmpPlayClientConnection` には、サーバー → クライアントへの `NetConnection.Connect.ReconnectRequest` を扱う経路が存在しない
- `onStatus` コマンド全般に対するクライアント側のディスパッチも、現状では限定的にしか扱われていない (要確認: `src/rtmp_client_connection.rs`)
- 既存の RtmpCommand 周辺 (`src/rtmp_command.rs`) では、`onStatus` の Info Object の汎用パースが行われていない可能性がある

## 設計方針

- Enhanced RTMP v2 の Reconnect Request セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 595 行以降) に準拠する
- クライアント側のフローを実装する
  1. サーバーから `onStatus` を受信し、Info Object の `code` が `NetConnection.Connect.ReconnectRequest`、`level` が `status` であることを確認する
  2. Info Object の `tcUrl` (optional)、`description` (optional) を取り出してイベントとしてユーザーに通知する
  3. 実際の再接続トリガはユーザー (上位アプリ) に委ねる (Sans I/O なので、本ライブラリは再接続フロー自体を駆動しない。次の媒体境界 (keyframe) までの継続配信と、新サーバーへの接続/旧サーバーからの切断は上位の責務)
- サーバー側の API を整備する
  - 任意のタイミングで `NetConnection.Connect.ReconnectRequest` を発行できるメソッドを提供する (tcUrl と description を任意指定可能)
- `RtmpConnectionEvent` に `ReconnectRequested { tcUrl: Option<String>, description: Option<String> }` を追加する
- 仕様の MUST 要件 (level=status、code=NetConnection.Connect.ReconnectRequest) を遵守する

## 完了条件

- クライアント側でサーバーからの `onStatus` を受信し、`code` が `NetConnection.Connect.ReconnectRequest` の場合に `RtmpConnectionEvent::ReconnectRequested` をユーザーに通知できる
- `tcUrl` および `description` が Info Object に含まれている場合は正しくパースされる
- サーバー側で任意のタイミングで `NetConnection.Connect.ReconnectRequest` の onStatus を発行できる API がある
- `tcUrl` の各種形式 (絶対 URI、相対 URI、`//host/path`、`/path`) を受信時に保持できる (URI 解決自体はユーザーに委ねる)
- ユニットテストで以下が確認できる
  - サーバーが送信した ReconnectRequest をクライアントが正しく受信できる
  - tcUrl あり/なしの双方のケース
  - 不正な level / code の場合は ReconnectRequested イベントを発火させない
- pbt の onStatus 用 Generator が拡張され、ReconnectRequest の round-trip プロパティが通る
- 既存の他 onStatus コードや connect / createStream / deleteStream の動作に回帰が無い

## 解決方法

1. `src/rtmp_command.rs`
   - `onStatus` コマンドの Info Object パース処理を必要に応じて拡張する (`code`、`level`、`description`、`tcUrl` を取り出せるようにする)
2. `src/rtmp_connection.rs` / `src/rtmp_client_connection.rs`
   - クライアント側で `onStatus` の受信時に Info Object の `code` を判定し、`NetConnection.Connect.ReconnectRequest` の場合に `RtmpConnectionEvent::ReconnectRequested` を発火する
3. `src/rtmp_server_connection.rs`
   - サーバー側で `NetConnection.Connect.ReconnectRequest` の onStatus を発行する API を追加する (引数: `tcUrl: Option<&str>`、`description: Option<&str>`)
4. `RtmpConnectionEvent`
   - `ReconnectRequested { tc_url: Option<String>, description: Option<String> }` バリアントを追加する
5. `src/lib.rs`
   - 新規型を re-export する
6. `tests/`
   - サーバー → クライアントの ReconnectRequest 往復テストを追加する
7. `pbt/`
   - onStatus の Generator を ReconnectRequest 対応に拡張する
8. `examples/`
   - サンプル (publish/server) で ReconnectRequest の送受信例を必要に応じて追加する
