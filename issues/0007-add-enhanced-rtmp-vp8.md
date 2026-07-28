# Enhanced RTMP の VP8 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-vp8
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Video 仕様のうち FourCC `vp08` に対応し、VP8 を含む映像フレームを送受信できるようにする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
HEVC (0004) / AV1 (0005) と比較すると、時雨堂製品での直接的な需要は相対的に低いため、優先度は Medium とする。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader 分岐、`VideoPacketType`、`VideoFourCc`、FourCC `avc1` 対応、`VideoFrame` のデータモデル再構成) は issue 0018 で導入される。本 issue はその基盤の上に `vp08` のペイロード処理を追加するスコープであり、0018 の完了が前提となる
- 0018 完了時点では、`vp08` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を VP8 実装で置き換える
- VP8 用シーケンスヘッダーは VP9 と同一の VPCodecConfigurationRecord (仕様 1322-1325 行) である。0006 (VP9) が `Vp9SequenceHeader` として実装するため、本 issue はこの型をそのまま再利用する。新規のシーケンスヘッダー型は不要。0006 の完了も前提となる

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`refs/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する
- 0018 で整備される ExVideoTagHeader 基盤の上に、FourCC が `vp08` (仕様 1218 行) の場合の分岐とペイロード処理を追加する
- 各 VideoPacketType の VP8 ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは VPCodecConfigurationRecord (仕様 1322-1325 行)。0006 の `Vp9SequenceHeader` を再利用してパース/シリアライズする
  - `CodedFrames`: ボディは coded full frames (仕様 1368-1371 行)。AVC/HEVC/VVC と異なり、VP8 の CodedFrames には compositionTimeOffset (SI24) が存在しない。ワイヤ上から SI24 を読まず、直ちにフレームデータが始まる。デコード時は `composition_timestamp_offset` に `RtmpTimestampDelta::ZERO` を設定し、エンコード時は同フィールドを無視する (pbt Generator も ZERO に拘束する)
  - `CodedFramesX`: VP8 には定義されていない (仕様 1411-1429 行の CodedFramesX セクションに VP8 エントリは存在しない)。デコード時に `vp08` + CodedFramesX を受信した場合は `Error::invalid_data` とする。エンコード時に `vp08` + CodedFramesX の `VideoFrame` を渡された場合も `Error::invalid_data` とする
  - `SequenceEnd`: ボディ無し (仕様 1317-1319 行)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0018 の avc1 分岐と同じ方針)
  - 上記以外の VideoPacketType (Metadata / MPEG2TSSequenceStart / Multitrack / ModEx) の扱いは 0018 のエラー分類に従う (それぞれ 0015 / 0005 / 0014 / 0015 で対応)
- 型の共有判断: VP8 と VP9 は同一の VPCodecConfigurationRecord を使用する (仕様 1322-1325 行と 1327-1330 行で同一構造)。0006 が定義する `Vp9SequenceHeader` を VP8 でもそのまま再利用する。リネーム (`VpCodecSequenceHeader` 等) は行わない (0006 の公開 API を変更しないため)。将来、命名の整合性が必要になったら別 issue で対応する
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 1212-1215 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる
- legacy VideoTagHeader で VP8 を送る非標準実装はスコープ外とする。本 issue が対象とするのは eRTMP の `vp08` のみ

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `vp08` の VideoTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / SequenceEnd)
- `vp08` + CodedFramesX のデコード/エンコードが `Error::invalid_data` になる
- SequenceStart のパースに 0006 の `Vp9SequenceHeader` が再利用でき、round-trip する
- ユニットテストで上記の各ケースが確認できる
- pbt: VP8 を含む `VideoFrame` の Generator が追加され、round-trip プロパティが通る
- fuzz: `VideoFrame` デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、VP8 のコーパス種を `fuzz/corpus/` に追加する。`Vp9SequenceHeader::from_bytes` の fuzz は 0006 で追加済みのため新設しない
- 既存の AVC (legacy / `avc1`) と他 legacy コーデックの動作に回帰が無い (既存テストがすべて通る)
- examples/server のシーケンスヘッダーキャッシュが VP8 の SequenceStart にも対応する (FFmpeg または OBS による手動確認でよい)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/flv.rs`
   - 0018 の FourCC 分岐で `vp08` を VP8 経路 (設計方針の各 VideoPacketType 処理) につなぐ。0018 で追加済みの `VideoCodec::ExVp8` variant を利用する (variant 自体の追加は 0018 のスコープ)
   - SequenceStart のパースに `Vp9SequenceHeader` を再利用する
   - `vp08` + CodedFramesX を `Error::invalid_data` で拒否する分岐を追加する
2. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・コーパスを追加する
3. `examples/server`
   - シーケンスヘッダーのキャッシュ判定を VP8 の SequenceStart にも対応させる。legacy AVC の SequenceHeader 判定も維持する
4. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
