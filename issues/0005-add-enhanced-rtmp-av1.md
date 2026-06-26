# Enhanced RTMP の AV1 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-av1
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Video 仕様 (FourCC `av01`) に対応し、AV1 を含む映像フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの映像コーデックカバレッジを拡張し、Sora など時雨堂製品との連携や、AV1 配信をサポートする主要配信ツール (FFmpeg、OBS Studio 30 系等) との相互接続を可能にする。

## 優先度根拠

High。eRTMP v2 仕様で AV1 は中心的なコーデックの 1 つ (HDR10+/HDR10/HLG/DV 全フォーマット対応) であり、本ライブラリが eRTMP 完全準拠を掲げる以上、優先実装すべき対象である。
HEVC (issue 0004) と同等の優先度で扱う。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader、VideoPacketType、VideoFourCc) は issue 0004 (HEVC) で導入される予定であり、本 issue はその基盤の上に AV1 のペイロード処理を追加するスコープ
- `VideoCodec` enum (`src/media.rs:120`) には AV1 のエントリが無く、AV1 用 Sequence Header に相当する構造体も存在しない
- AV1 は MPEG2TSSequenceStart にも対応するため、SequenceStart の 2 系統を扱う必要がある

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`docs/enhanced/enhanced-rtmp-v2.md` の AV1 関連記述、1332/1361/1378 行付近) に準拠する
- 0004 (HEVC) で整備される ExVideoTagHeader 基盤の上に、AV1 用の分岐とペイロード処理を追加する
- AV1 用 Sequence Header は AV1CodecConfigurationRecord (AV1-ISOBMFF specification) として表現する。`AvcSequenceHeader` / `HevcSequenceHeader` と同じ粒度で `from_bytes` / `to_bytes` を提供する
- VideoPacketType の `MPEG2TSSequenceStart` (AV1VideoDescriptor を含む) も AV1 のために本 issue で対応する。他コーデックでは未対応のまま (将来必要になったら別 issue)
- `CodedFrames` / `CodedFramesX` における AV1 ペイロードは「1 つ以上の OBU からなる単一 temporal unit」として扱う (compositionTimeOffset は AV1 では明示されないが、ExVideoTagHeader の規定どおり `CodedFrames` 経路では SI24 を読む。仕様の AV1 経路では明示されていないが、HEVC/VVC と異なり、AV1 の `CodedFrames` ブロックでは compositionTimeOffset は読まれない点に注意)
- 未対応の VideoFourCc を受信した場合は `Error::unsupported` を返す (本 issue で AV1 を Supported に加える)

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `av01` の VideoTagHeader をデコード/エンコードできる
- VideoPacketType の `SequenceStart` / `CodedFrames` / `CodedFramesX` / `SequenceEnd` / `MPEG2TSSequenceStart` を AV1 ペイロードと正しく組み合わせて扱える
- AV1CodecConfigurationRecord のパース/シリアライズが round-trip する
- ユニットテストで以下が確認できる
  - AV1 の VideoTagHeader (各 VideoPacketType) の round-trip
  - `Av1SequenceHeader::from_bytes` / `to_bytes` の round-trip
- pbt の AV1 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている (`Av1SequenceHeader::from_bytes`、AV1 を含む VideoFrame デコード)
- 既存の AVC / HEVC / legacy コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `VideoCodec` enum (または 0004 で導入される FourCC 用統合表現) に `Av1` を追加する
   - `Av1SequenceHeader` (AV1CodecConfigurationRecord) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - `decode_video_frame` / `encode_video_frame` で AV1 ペイロード経路を扱う
   - `MPEG2TSSequenceStart` の AV1VideoDescriptor を読み書きする
3. `src/lib.rs`
   - `Av1SequenceHeader` 等の新規型を re-export する
4. `tests/`
   - AV1 の round-trip テストを追加する
5. `pbt/`
   - AV1 用 Generator を追加する
6. `fuzz/`
   - `Av1SequenceHeader::from_bytes` と AV1 VideoFrame デコードの fuzz ターゲットを追加する
