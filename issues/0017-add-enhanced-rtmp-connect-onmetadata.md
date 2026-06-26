# Enhanced RTMP の connect コマンド / onMetaData 拡張対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-connect-onmetadata
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の connect コマンド拡張 (`fourCcList` / `videoFourCcInfoMap` / `audioFourCcInfoMap` / `capsEx`) と onMetaData 拡張 (`audiocodecid` / `videocodecid` の FourCC 値、`audioTrackIdInfoMap` / `videoTrackIdInfoMap`) に対応する。
これにより、eRTMP 完全準拠を目指す本ライブラリで、クライアントとサーバーの間で eRTMP 拡張機能の能力ネゴシエーション (codec / Reconnect / Multitrack / ModEx) と、配信メディアのコーデック・トラック構成のメタデータ伝達が正しく行えるようになる。

## 優先度根拠

High。eRTMP 仕様で「FourCC コーデックへのサポートは MUST `connect` コマンド経由で表明する」と明示的に規定されており (`docs/enhanced/enhanced-rtmp-v2.md` の 849 行)、Enhanced Video / Enhanced Audio の全コーデック対応 issue (0004-0013) はこの能力ネゴシエーションが揃って初めて完全準拠と言える。
0014 Multitrack や 0016 Reconnect Request も `capsEx` 経由の能力宣言とセットで完成する。

## 現状

- connect コマンドの Command Object パース (`src/rtmp_command.rs` 周辺) は legacy の最小プロパティのみを扱っており、`fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx` のいずれも認識しない
- onMetaData のパース (要確認: `src/rtmp_command.rs` または scriptdata 処理箇所) も、`audiocodecid` / `videocodecid` を legacy CodecID 値としてのみ扱い、FourCC UI32 値を解釈できない
- `audioTrackIdInfoMap` / `videoTrackIdInfoMap` の概念がコードベースに存在しない

## 設計方針

- Enhanced RTMP v2 の Enhancing NetConnection connect Command セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 1672 行以降) と Enhancing onMetaData セクション (461 行以降) に準拠する
- 本 issue では以下の 2 群をまとめてスコープに含める (両者はクライアント能力宣言とメタデータ表現として一体的に扱う必要があるため)
  - connect コマンドの新規プロパティ: `fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx`
  - onMetaData の拡張: `audiocodecid` / `videocodecid` の FourCC 値、`audioTrackIdInfoMap` / `videoTrackIdInfoMap`
- `FourCcInfoMask` (CanDecode / CanEncode / CanForward) のビットマスク型を定義する
- `CapsExMask` (Reconnect / Multitrack / ModEx / TimestampNanoOffset) のビットマスク型を定義する
- ワイルドカード `"*"` の扱いを実装する (任意の codec を catch-all で表現)
- クライアント側で connect コマンドを発行する API に、これらのプロパティを設定できるオプションを追加する
- サーバー側で connect コマンドを受信した際に、これらのプロパティを抽出できるイベントまたはアクセサを追加する
- サーバー側で `_result` / `_error` レスポンスにも `videoFourCcInfoMap` / `capsEx` 等を含められるようにする (仕様 1753 行)
- onMetaData の `audiocodecid` / `videocodecid` は値の型 (number) を維持しつつ、FourCC を big-endian UI32 として表現する経路を追加する
- `audioTrackIdInfoMap` / `videoTrackIdInfoMap` の各エントリの自由なメタデータ (width / height / videodatarate / channels / samplerate / codec identifier 等) は AMF Object として保持する。本ライブラリでは固定キーを強制せず、ユーザーが必要に応じて取り出せるようにする
- 仕様の RECOMMENDED 「クライアント側は `fourCcList` から `[audio|video]FourCcInfoMap` への移行を推奨」「サーバー側は両方の併用サポートを推奨」を踏まえ、本ライブラリは両者を扱える設計とする

## 完了条件

- connect コマンドの Command Object で以下が読み書きできる
  - `fourCcList` (Strict Array of strings / FourCC、`"*"` ワイルドカード対応)
  - `videoFourCcInfoMap` / `audioFourCcInfoMap` (FourCC -> FourCcInfoMask、`"*"` ワイルドカード対応)
  - `capsEx` (CapsExMask の bitwise OR)
- connect コマンドの `_result` / `_error` レスポンスでも上記同様のプロパティを読み書きできる
- onMetaData の `audiocodecid` / `videocodecid` を FourCC 値として読み書きできる (legacy CodecID 値との両立)
- onMetaData の `audioTrackIdInfoMap` / `videoTrackIdInfoMap` を AMF Object として読み書きできる
- ユニットテストで以下が確認できる
  - connect コマンドの拡張プロパティの round-trip
  - `_result` レスポンスの拡張プロパティの round-trip
  - onMetaData の FourCC codec id の往復
  - onMetaData の TrackIdInfoMap の往復
  - ワイルドカード `"*"` の扱い
- pbt の connect コマンド / onMetaData 用 Generator が拡張され、round-trip プロパティが通る
- 既存の legacy connect / onMetaData 処理に回帰が無い

## 解決方法

1. `src/rtmp_command.rs`
   - connect コマンドの Command Object パースに `fourCcList`、`videoFourCcInfoMap`、`audioFourCcInfoMap`、`capsEx` プロパティの読み書きを追加する
   - `_result` / `_error` レスポンスのオブジェクトにも同様のプロパティを読み書きできるようにする
   - onMetaData のパースで `audiocodecid` / `videocodecid` を number の big-endian UI32 として FourCC 解釈できる経路を追加する
   - onMetaData の `audioTrackIdInfoMap` / `videoTrackIdInfoMap` を AMF Object として保持する
2. `src/media.rs` (または新規ファイル)
   - `FourCcInfoMask` (CanDecode / CanEncode / CanForward) ビットマスク型を定義する
   - `CapsExMask` (Reconnect / Multitrack / ModEx / TimestampNanoOffset) ビットマスク型を定義する
3. `src/rtmp_client_connection.rs`
   - connect コマンド発行時に上記プロパティを設定できる API を追加する (RtmpPublishClientConnection / RtmpPlayClientConnection 共通)
4. `src/rtmp_server_connection.rs`
   - connect コマンド受信時に上記プロパティを抽出できる API を追加する
   - `_result` 発行時に上記プロパティを設定できる API を追加する
5. `src/lib.rs`
   - 新規型を re-export する
6. `tests/`
   - connect コマンド拡張の round-trip テストを追加する
   - onMetaData 拡張の round-trip テストを追加する
   - ワイルドカード `"*"` の扱いを確認するテストを追加する
7. `pbt/`
   - connect コマンド / onMetaData 用 Generator を拡張する
8. `examples/`
   - サンプル (publish/server) で connect コマンドの eRTMP 拡張プロパティを設定する例を必要に応じて追加する
