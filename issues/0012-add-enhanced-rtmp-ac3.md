# Enhanced RTMP の AC-3 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-ac3
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Audio 仕様 (FourCC `ac-3`) に対応し、AC-3 (Dolby Digital) を含む音声フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの音声コーデックカバレッジを拡張する。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
AC-3 は放送・配信領域で広く使われるが、Sora など時雨堂製品での直接的な需要は現時点で限定的なため、優先度は Medium とする。

## 現状

- ExAudioTagHeader 基盤は issue 0009 (Enhanced Audio 基盤) で導入される予定であり、本 issue はその基盤の上に AC-3 のペイロード処理を追加するスコープ
- 既存の `AudioFormat` enum (`src/media.rs:144`) には AC-3 のエントリが無い

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`docs/enhanced/enhanced-rtmp-v2.md` の AC-3 関連記述、1025 行付近) に準拠する
- 0009 (Enhanced Audio 基盤) で整備される ExAudioTagHeader 基盤の上に、AC-3 用の分岐とペイロード処理を追加する
- AC-3 は仕様上 SequenceStart のペイロードが定義されていない。`CodedFrames` のみに ATSC A/52 ビットストリーム (Ac3CodedData) を持つ
- MultichannelConfig との連携を許容する (codec が self-describing for channel mapping ではない場合に必要)

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `ac-3` の AudioTagHeader をデコード/エンコードできる
- AudioPacketType の `CodedFrames` / `SequenceEnd` を AC-3 ペイロードと正しく組み合わせて扱える
- ユニットテストで AC-3 の round-trip が確認できる
- pbt の AC-3 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている (AC-3 AudioFrame デコード)
- 既存の legacy / 他の Enhanced Audio コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `AudioFourCc` enum (0009 で導入) の `Ac3` バリアントを使う
2. `src/flv.rs`
   - ExAudioTagHeader 経路で AudioFourCc が `ac-3` の場合のペイロード分岐を追加する
3. `tests/`
   - AC-3 の round-trip テストを追加する
4. `pbt/`
   - AC-3 用 Generator を追加する
5. `fuzz/`
   - AC-3 AudioFrame デコードの fuzz ターゲットを追加する
