# Enhanced RTMP の connect コマンド / onMetaData 拡張対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-connect-onmetadata
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の connect コマンド拡張 (`fourCcList` / `videoFourCcInfoMap` / `audioFourCcInfoMap` / `capsEx`) と onMetaData 拡張 (`audiocodecid` / `videocodecid` の FourCC 値、`audioTrackIdInfoMap` / `videoTrackIdInfoMap`) に対応する。

## 優先度根拠

High。eRTMP 仕様で「FourCC コーデックへのサポートは MUST `connect` コマンド経由で表明する」と明示的に規定されており (`refs/enhanced-rtmp-v2.md` の 849 行)、Enhanced Video / Enhanced Audio の全コーデック対応 issue (0004-0013) はこの能力ネゴシエーションが揃って初めて完全準拠と言える。
0014 Multitrack や 0016 Reconnect Request も `capsEx` 経由の能力宣言とセットで完成する。
0009 / 0018 はともに「connect コマンドでの FourCC signaling は 0017 で対応する」と明記しており、本 issue がブロックしている。

## 現状

- connect コマンドの Command Object パース (`src/rtmp_command.rs` 周辺) は legacy の最小プロパティのみを扱っており、`fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx` のいずれも認識しない
- onMetaData のパース処理はコードベースに一切存在しない。`RtmpMessage::Data` variant は `src/rtmp_message.rs:135` に存在するが、クライアント (`src/rtmp_client_connection.rs:359-363`)・サーバー (`src/rtmp_server_connection.rs:145-148`) ともに Data メッセージは `message_ignored` で捨てている。本 issue は onMetaData の新規実装を行う
- `audioTrackIdInfoMap` / `videoTrackIdInfoMap` の概念がコードベースに存在しない
- `RtmpConnectCommand::accept()` (`src/rtmp_command.rs:267-291`) は `_result` の properties を完全ハードコード (`fmsVer`, `capabilities`, `mode`) しており、拡張プロパティを含められない

## 設計方針

- Enhanced RTMP v2 の Enhancing NetConnection connect Command セクション (`refs/enhanced-rtmp-v2.md` の 1672 行以降) と Enhancing onMetaData セクション (461 行以降) に準拠する
- 本 issue では以下の 2 群をまとめてスコープに含める。connect コマンド拡張と onMetaData 拡張はプロトコルレイヤーが異なる (Command Message vs Data Message) が、onMetaData 側のスコープが小さく (FourCC 解釈 + TrackIdInfoMap の AMF Object 保持)、分割すると issue が過剰に細かくなるため統合する:
  - connect コマンドの新規プロパティ: `fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx`
  - onMetaData の拡張: `audiocodecid` / `videocodecid` の FourCC 値、`audioTrackIdInfoMap` / `videoTrackIdInfoMap`
- connect コマンド拡張:
  - `FourCcInfoMask` (CanDecode / CanEncode / CanForward) のビットマスク型を `src/rtmp_command.rs` に定義する (connect コマンドのシグナリング型であり、メディアフレームの型ではないため media.rs には置かない)
  - `CapsExMask` (Reconnect / Multitrack / ModEx / TimestampNanoOffset) のビットマスク型を `src/rtmp_command.rs` に定義する
  - ワイルドカード `"*"` の扱いを実装する (任意の codec を catch-all で表現)
  - クライアント側で connect コマンドを発行する API に、これらのプロパティを設定できるオプションを追加する。現状 `RtmpPublishClientConnection::new(url)` / `RtmpPlayClientConnection::new(url)` は URL のみ受け取るため、connect 拡張プロパティを保持するオプション構造体 (`ConnectOptions` 等) を導入し、`new()` の引数に追加する
  - サーバー側で connect コマンドを受信した際に、これらのプロパティを抽出できるイベントまたはアクセサを追加する
  - サーバー側で `_result` レスポンスに `videoFourCcInfoMap` / `capsEx` 等を含められるようにする (仕様 1753 行。SHOULD であり MUST ではない)。`RtmpConnectCommand::accept()` のハードコードを拡張し、オプションで拡張プロパティを含められるようにする
- onMetaData 拡張:
  - onMetaData は RTMP の Data Message (message type 18/15) であり、Command Message (type 20/17) ではない。`RtmpCommand` enum には追加せず、`src/rtmp_message.rs` の `RtmpMessage::Data` 処理経路に新規ハンドラを追加する。AMF0 の SCRIPTDATA としてパースし、`RtmpConnectionEvent::MetaData` 等の新規イベントでユーザーに通知する
  - `audiocodecid` / `videocodecid` は値の型 (number) を維持しつつ、FourCC を big-endian UI32 として表現する経路を追加する (例: "Opus" == 0x4F707573 == 1332770163.0)
  - `audioTrackIdInfoMap` / `videoTrackIdInfoMap` の各エントリの自由なメタデータ (width / height / videodatarate / channels / samplerate / codec identifier 等) は AMF Object として保持する。本ライブラリでは固定キーを強制せず、ユーザーが必要に応じて取り出せるようにする
- 仕様の RECOMMENDED 「クライアント側は `fourCcList` から `[audio|video]FourCcInfoMap` への移行を推奨」「サーバー側は両方の併用サポートを推奨」を踏まえ、本ライブラリは両者を扱える設計とする

## 完了条件

- connect コマンドの Command Object で以下が読み書きできる
  - `fourCcList` (Strict Array of strings / FourCC、`"*"` ワイルドカード対応)
  - `videoFourCcInfoMap` / `audioFourCcInfoMap` (FourCC -> FourCcInfoMask、`"*"` ワイルドカード対応)
  - `capsEx` (CapsExMask の bitwise OR)
- connect コマンドの `_result` レスポンスでも上記同様のプロパティを読み書きできる
- onMetaData の `audiocodecid` / `videocodecid` を FourCC 値として読み書きできる (legacy CodecID 値との両立)
- onMetaData の `audioTrackIdInfoMap` / `videoTrackIdInfoMap` を AMF Object として読み書きできる
- ユニットテストで以下が確認できる
  - connect コマンドの拡張プロパティの round-trip
  - `_result` レスポンスの拡張プロパティの round-trip
  - onMetaData の FourCC codec id の往復
  - onMetaData の TrackIdInfoMap の往復
  - ワイルドカード `"*"` の扱い
- pbt: connect コマンド用 Generator を拡張し、onMetaData 用 Generator を新規追加する。round-trip プロパティが通る
- 既存の legacy connect 処理に回帰が無い
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/rtmp_command.rs`
   - `FourCcInfoMask` (CanDecode / CanEncode / CanForward) ビットマスク型を定義する
   - `CapsExMask` (Reconnect / Multitrack / ModEx / TimestampNanoOffset) ビットマスク型を定義する
   - connect コマンドの Command Object パースに `fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx` プロパティの読み書きを追加する
   - `_result` レスポンスのオブジェクトにも同様のプロパティを読み書きできるようにする
   - `RtmpConnectCommand::accept()` を拡張し、オプションで拡張プロパティを含められるようにする
2. `src/rtmp_message.rs`
   - `RtmpMessage::Data` の処理経路に onMetaData ハンドラを追加する (AMF0 SCRIPTDATA としてパース)
   - onMetaData の `audiocodecid` / `videocodecid` を FourCC 解釈できる経路を追加する
   - onMetaData の `audioTrackIdInfoMap` / `videoTrackIdInfoMap` を AMF Object として保持する
3. `src/rtmp_client_connection.rs`
   - connect コマンド発行時に拡張プロパティを設定できる API を追加する (`ConnectOptions` 構造体等を導入)
   - Data メッセージ受信時に onMetaData を `RtmpConnectionEvent` として通知する経路を追加する
4. `src/rtmp_server_connection.rs`
   - connect コマンド受信時に拡張プロパティを抽出できる API を追加する
   - `_result` 発行時に拡張プロパティを設定できる API を追加する
5. `src/rtmp_connection.rs`
   - `RtmpConnectionEvent` に onMetaData 通知用のバリアントを追加する
6. `src/lib.rs`
   - 新規型 (`FourCcInfoMask` / `CapsExMask` / `ConnectOptions` 等) を re-export する
7. `tests/` / `pbt/`
   - 完了条件に列挙したテスト・Generator を追加する
8. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
