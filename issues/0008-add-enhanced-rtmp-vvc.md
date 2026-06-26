# Enhanced RTMP の VVC (H.266) 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-vvc
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Video 仕様 (FourCC `vvc1`) に対応し、VVC (H.266) を含む映像フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの映像コーデックカバレッジを拡張する。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
VVC は次世代コーデック (HEVC の後継) として位置付けられているが、現時点で配信側ツールでの採用は限定的なため、優先度は Medium とする。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader、VideoPacketType、VideoFourCc) は issue 0004 (HEVC) で導入される予定であり、本 issue はその基盤の上に VVC のペイロード処理を追加するスコープ
- `VideoCodec` enum (`src/media.rs:120`) には VVC のエントリが無く、VVC 用 Sequence Header に相当する構造体も存在しない

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`docs/enhanced/enhanced-rtmp-v2.md` の VVC 関連記述、1351/1401 行付近) に準拠する
- 0004 (HEVC) で整備される ExVideoTagHeader 基盤の上に、VVC 用の分岐とペイロード処理を追加する
- VVC 用 Sequence Header は VVCDecoderConfigurationRecord (ISO/IEC 14496-15:2024, 11.2.4.2) として表現する。`HevcSequenceHeader` と同じ粒度で `from_bytes` / `to_bytes` を提供する
- `CodedFrames` における VVC ペイロードは「1 つ以上の NALU、full frames が必須」として扱う。先頭に SI24 compositionTimeOffset が付く
- `CodedFramesX` における VVC ペイロードは NALU のみ (compositionTimeOffset=0 暗黙)

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `vvc1` の VideoTagHeader をデコード/エンコードできる
- VideoPacketType の `SequenceStart` / `CodedFrames` / `CodedFramesX` / `SequenceEnd` を VVC ペイロードと正しく組み合わせて扱える
- VVCDecoderConfigurationRecord のパース/シリアライズが round-trip する
- ユニットテストで VVC の round-trip が確認できる
- pbt の VVC 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
- 既存の AVC / HEVC / AV1 / VP9 / VP8 / legacy コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `VideoCodec` enum (または 0004 で導入される FourCC 用統合表現) に `Vvc` を追加する
   - `VvcSequenceHeader` (VVCDecoderConfigurationRecord) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - `decode_video_frame` / `encode_video_frame` で VVC ペイロード経路を扱う
3. `src/lib.rs`
   - `VvcSequenceHeader` 等の新規型を re-export する
4. `tests/`
   - VVC の round-trip テストを追加する
5. `pbt/`
   - VVC 用 Generator を追加する
6. `fuzz/`
   - `VvcSequenceHeader::from_bytes` と VVC VideoFrame デコードの fuzz ターゲットを追加する
