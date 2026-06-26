# Enhanced RTMP の VP8 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-vp8
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Video 仕様 (FourCC `vp08`) に対応し、VP8 を含む映像フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの映像コーデックカバレッジを拡張する。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
VP9 と同じく、Sora など時雨堂製品での直接的な需要は現時点で低いため、優先度は Medium とする。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader、VideoPacketType、VideoFourCc) は issue 0004 (HEVC) で導入される予定であり、本 issue はその基盤の上に VP8 のペイロード処理を追加するスコープ
- `VideoCodec` enum (`src/media.rs:120`) には VP8 のエントリが無く、VP8 用 Sequence Header に相当する構造体も存在しない

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`docs/enhanced/enhanced-rtmp-v2.md` の VP8 関連記述、1322/1368 行付近) に準拠する
- 0004 (HEVC) で整備される ExVideoTagHeader 基盤の上に、VP8 用の分岐とペイロード処理を追加する
- VP8 用 Sequence Header は VPCodecConfigurationRecord として表現する (VP9 と同じレコード形式)
- `CodedFrames` における VP8 ペイロードは「coded full frames」として扱う

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `vp08` の VideoTagHeader をデコード/エンコードできる
- VideoPacketType の `SequenceStart` / `CodedFrames` / `SequenceEnd` を VP8 ペイロードと正しく組み合わせて扱える
- VPCodecConfigurationRecord (VP8 用) のパース/シリアライズが round-trip する
- ユニットテストで VP8 の round-trip が確認できる
- pbt の VP8 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
- 既存の AVC / HEVC / AV1 / VP9 / legacy コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `VideoCodec` enum (または 0004 で導入される FourCC 用統合表現) に `Vp8` を追加する
   - `Vp8SequenceHeader` を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する。VP9 と共通の VPCodecConfigurationRecord 表現がある場合は共通型として整理する
2. `src/flv.rs`
   - `decode_video_frame` / `encode_video_frame` で VP8 ペイロード経路を扱う
3. `src/lib.rs`
   - 新規型を re-export する
4. `tests/`
   - VP8 の round-trip テストを追加する
5. `pbt/`
   - VP8 用 Generator を追加する
6. `fuzz/`
   - `Vp8SequenceHeader::from_bytes` と VP8 VideoFrame デコードの fuzz ターゲットを追加する
