# Enhanced RTMP の Reconnect Request 対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-reconnect-request
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Reconnect Request 仕様 (`NetConnection.Connect.ReconnectRequest`) に対応し、サーバーがクライアントに再接続を促す `onStatus` を扱えるようにする。

## 優先度根拠

High。eRTMP v2 仕様で `CapsExMask.Reconnect` フラグでクライアント能力として明示的に宣言される機能であり、完全準拠を掲げる以上、対応必須。
ライブ配信サーバーのローリングアップデートや、Sora 等の SFU 接続先の再マッピングといった運用シナリオで効果的に機能する。

## 現状

- `RtmpOnStatusCommand` (`src/rtmp_command.rs:534-570`) は `level` / `code` / `description` / `details` をパース済みだが、`tcUrl` は未対応
- サーバー → クライアントの `NetConnection.Connect.ReconnectRequest` を `RtmpConnectionEvent` として通知する経路が存在しない
- サーバー側から `NetConnection.Connect.ReconnectRequest` の onStatus を発行する API が存在しない

## 設計方針

- Enhanced RTMP v2 の Reconnect Request セクション (`refs/enhanced-rtmp-v2.md` の 595 行以降) に準拠する
- クライアント側のフロー:
  1. サーバーから `onStatus` を受信し、Info Object の `code` が `NetConnection.Connect.ReconnectRequest`、`level` が `status` であることを確認する
  2. Info Object の `tcUrl` (optional)、`description` (optional) を取り出してイベントとしてユーザーに通知する
  3. 実際の再接続トリガはユーザー (上位アプリ) に委ねる (Sans I/O なので、本ライブラリは再接続フロー自体を駆動しない)
- サーバー側の API:
  - 任意のタイミングで `NetConnection.Connect.ReconnectRequest` を発行できるメソッドを提供する (引数: `tc_url: Option<&str>`、`description: Option<&str>`)
  - 仕様 661 行の MUST 要件「If the server aims to remap the client, it MUST set the tcUrl property」は API ドキュメント (rustdoc) で注記する。ライブラリとして tcUrl の設定を強制しない (Sans I/O の設計判断)
- `RtmpConnectionEvent` に `ReconnectRequested { tc_url: Option<String>, description: Option<String> }` バリアントを追加する (既存の `PublishRequested` が `tc_url` を使っているため、命名は `tc_url` に統一)
- `RtmpOnStatusCommand` に `tc_url: Option<String>` フィールドを追加し、`from_message` のパースと `into_message` のシリアライズの両方に対応する
- Info Object の未知プロパティは破棄する (既存方針を維持。仕様 635 行「It MAY contain other properties」)
- 0017 (connect コマンド拡張) との関係: Reconnect 能力は `capsEx` で宣言される (仕様 665 行) が、0016 は 0017 の完了を待たずに実装・完了してよい。`capsEx` の判定は 0017 のスコープであり、0016 は ReconnectRequest の送受信プリミティブのみを提供する。クライアントが `capsEx` で Reconnect 能力を宣言していない場合でも、サーバーからの ReconnectRequest はイベントとして通知する (能力判定は上位アプリの責務)

## 完了条件

- クライアント側でサーバーからの `onStatus` を受信し、`code` が `NetConnection.Connect.ReconnectRequest` の場合に `RtmpConnectionEvent::ReconnectRequested` をユーザーに通知できる
- `tcUrl` および `description` が Info Object に含まれている場合は正しくパースされる
- サーバー側で任意のタイミングで `NetConnection.Connect.ReconnectRequest` の onStatus を発行できる API がある
- `tcUrl` を文字列としてそのまま保持し、URI 解決やバリデーションは行わない
- 不正な level / code の場合は ReconnectRequested イベントを発火させない
- ユニットテストで以下が確認できる
  - サーバーが送信した ReconnectRequest をクライアントが正しく受信できる
  - tcUrl あり/なしの双方のケース
  - 不正な level / code の場合は ReconnectRequested イベントを発火させない
- pbt: onStatus の round-trip テストを新規追加する (既存の pbt に onStatus の Generator は存在しないため)
- 既存の他 onStatus コードや connect / createStream / deleteStream の動作に回帰が無い
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/rtmp_command.rs`
   - `RtmpOnStatusCommand` に `tc_url: Option<String>` フィールドを追加する
   - `from_message` のパース処理に `tcUrl` プロパティの読み取りを追加する
   - `into_message` のシリアライズ処理に `tcUrl` プロパティの書き出しを追加する
2. `src/rtmp_connection.rs`
   - `RtmpConnectionEvent` に `ReconnectRequested { tc_url: Option<String>, description: Option<String> }` バリアントを追加する
3. `src/rtmp_client_connection.rs`
   - クライアント側で `onStatus` の受信時に Info Object の `code` を判定し、`NetConnection.Connect.ReconnectRequest` の場合に `RtmpConnectionEvent::ReconnectRequested` を発火する
4. `src/rtmp_server_connection.rs`
   - サーバー側で `NetConnection.Connect.ReconnectRequest` の onStatus を発行する API を追加する
5. `tests/` / `pbt/`
   - 完了条件に列挙したテスト・pbt を追加する
6. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
