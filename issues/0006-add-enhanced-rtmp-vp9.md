# Enhanced RTMP の VP9 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-vp9
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Video 仕様 (FourCC `vp09`) に対応し、VP9 を含む映像フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの映像コーデックカバレッジを拡張し、VP9 配信をサポートする主要配信ツールとの相互接続を可能にする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
ただし、HEVC (0004) / AV1 (0005) と比較すると、Sora など時雨堂製品での直接的な需要は現時点で低いため、優先度は Medium とする。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader、VideoPacketType、VideoFourCc) は issue 0004 (HEVC) で導入される予定であり、本 issue はその基盤の上に VP9 のペイロード処理を追加するスコープ
- `VideoCodec` enum (`src/media.rs:120`) には VP9 のエントリが無く、VP9 用 Sequence Header に相当する構造体も存在しない

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`docs/enhanced/enhanced-rtmp-v2.md` の VP9 関連記述、1327/1373 行付近) に準拠する
- 0004 (HEVC) で整備される ExVideoTagHeader 基盤の上に、VP9 用の分岐とペイロード処理を追加する
- VP9 用 Sequence Header は VPCodecConfigurationRecord として表現する。`AvcSequenceHeader` / `HevcSequenceHeader` と同じ粒度で `from_bytes` / `to_bytes` を提供する
- `CodedFrames` / `CodedFramesX` における VP9 ペイロードは「coded full frames」として扱う

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `vp09` の VideoTagHeader をデコード/エンコードできる
- VideoPacketType の `SequenceStart` / `CodedFrames` / `CodedFramesX` / `SequenceEnd` を VP9 ペイロードと正しく組み合わせて扱える
- VPCodecConfigurationRecord のパース/シリアライズが round-trip する
- ユニットテストで VP9 の round-trip が確認できる
- pbt の VP9 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
- 既存の AVC / HEVC / AV1 / legacy コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `VideoCodec` enum (または 0004 で導入される FourCC 用統合表現) に `Vp9` を追加する
   - `Vp9SequenceHeader` (VPCodecConfigurationRecord) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - `decode_video_frame` / `encode_video_frame` で VP9 ペイロード経路を扱う
3. `src/lib.rs`
   - `Vp9SequenceHeader` 等の新規型を re-export する
4. `tests/`
   - VP9 の round-trip テストを追加する
5. `pbt/`
   - VP9 用 Generator を追加する
6. `fuzz/`
   - `Vp9SequenceHeader::from_bytes` と VP9 VideoFrame デコードの fuzz ターゲットを追加する
