# Enhanced RTMP の FLAC 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-flac
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Audio 仕様 (FourCC `fLaC`) に対応し、FLAC を含む音声フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの音声コーデックカバレッジを拡張する。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
FLAC はロスレス音声の代表として位置付けられるが、Sora など時雨堂製品での直接的な需要は現時点で限定的なため、優先度は Medium とする。

## 現状

- ExAudioTagHeader 基盤は issue 0009 (Enhanced Audio 基盤) で導入される予定であり、本 issue はその基盤の上に FLAC のペイロード処理を追加するスコープ
- 既存の `AudioFormat` enum (`src/media.rs:144`) には FLAC のエントリが無く、FLAC 用 Sequence Header に相当する構造体も存在しない

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`docs/enhanced/enhanced-rtmp-v2.md` の FLAC 関連記述、995/1061 行付近) に準拠する
- 0009 (Enhanced Audio 基盤) で整備される ExAudioTagHeader 基盤の上に、FLAC 用の分岐とペイロード処理を追加する
- FLAC 用 Sequence Header は FlacSequenceHeader (FLAC `fLaC` シグネチャ + STREAMINFO メタデータブロック) として表現する。`from_bytes` / `to_bytes` を提供する
- `CodedFrames` における FLAC ペイロードは「FLAC 仕様の音声フレーム列」として扱う

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `fLaC` の AudioTagHeader をデコード/エンコードできる
- AudioPacketType の `SequenceStart` / `CodedFrames` / `SequenceEnd` を FLAC ペイロードと正しく組み合わせて扱える
- FlacSequenceHeader (fLaC + STREAMINFO) のパース/シリアライズが round-trip する
- ユニットテストで FLAC の round-trip が確認できる
- pbt の FLAC 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
- 既存の legacy / 他の Enhanced Audio コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `AudioFourCc` enum (0009 で導入) の `Flac` バリアントを使う
   - `FlacSequenceHeader` (fLaC シグネチャ + STREAMINFO) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - ExAudioTagHeader 経路で AudioFourCc が `fLaC` の場合のペイロード分岐を追加する
3. `src/lib.rs`
   - `FlacSequenceHeader` 等の新規型を re-export する
4. `tests/`
   - FLAC の round-trip テストを追加する
5. `pbt/`
   - FLAC 用 Generator を追加する
6. `fuzz/`
   - `FlacSequenceHeader::from_bytes` と FLAC AudioFrame デコードの fuzz ターゲットを追加する
