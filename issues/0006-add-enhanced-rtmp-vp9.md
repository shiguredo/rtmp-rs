# Enhanced RTMP の VP9 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-vp9
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Video 仕様のうち FourCC `vp09` に対応し、VP9 を含む映像フレームを送受信できるようにする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
HEVC (0004) / AV1 (0005) と比較すると、時雨堂製品での直接的な需要は相対的に低いため、優先度は Medium とする。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader 分岐、`VideoPacketType`、`VideoFourCc`、FourCC `avc1` 対応、`VideoFrame` のデータモデル再構成) は issue 0018 で導入される。本 issue はその基盤の上に `vp09` のペイロード処理を追加するスコープであり、0018 の完了が前提となる
- 0018 完了時点では、`vp09` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を VP9 実装で置き換える
- VP9 用シーケンスヘッダー (VPCodecConfigurationRecord) に対応する型は存在しない。AVC 用には `AvcSequenceHeader` (`src/media.rs:226`) が AVCDecoderConfigurationRecord のパース/シリアライズを提供しており、本 issue はこれと同等の型を VP9 用に追加する

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`refs/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する
- 0018 で整備される ExVideoTagHeader 基盤の上に、FourCC が `vp09` (仕様 1219 行) の場合の分岐とペイロード処理を追加する
- 各 VideoPacketType の VP9 ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは VPCodecConfigurationRecord (仕様 1327-1330 行)
  - `CodedFrames`: ボディは coded full frames (仕様 1373-1376 行)。AVC/HEVC/VVC と異なり、VP9 の CodedFrames には compositionTimeOffset (SI24) が存在しない。ワイヤ上から SI24 を読まず、直ちにフレームデータが始まる。デコード時は `composition_timestamp_offset` に `RtmpTimestampDelta::ZERO` を設定し、エンコード時は同フィールドを無視する (pbt Generator も ZERO に拘束する)
  - `CodedFramesX`: VP9 には定義されていない (仕様 1411-1429 行の CodedFramesX セクションに VP9 エントリは存在しない)。デコード時に `vp09` + CodedFramesX を受信した場合は `Error::invalid_data` とする。エンコード時に `vp09` + CodedFramesX の `VideoFrame` を渡された場合も `Error::invalid_data` とする
  - `SequenceEnd`: ボディ無し (仕様 1317-1319 行)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0018 の avc1 分岐と同じ方針)
  - 上記以外の VideoPacketType (Metadata / MPEG2TSSequenceStart / Multitrack / ModEx) の扱いは 0018 のエラー分類に従う (それぞれ 0015 / 0005 / 0014 / 0015 で対応)
- VPCodecConfigurationRecord を `Vp9SequenceHeader` として表現し、`AvcSequenceHeader` と同様に `from_bytes` / `to_bytes` を提供する。VP Codec ISO Media File Format Binding (VP-BITSTREAM, Section "VPCodecConfigurationRecord") は `refs/` に存在しないため、以下のレイアウトを照合先とする:
  - 固定部 (8 バイト): profile (u8) + level (u8) + bitDepth (4bit) + chromaSubsampling (3bit) + videoFullRangeFlag (1bit) + colourPrimaries (u8) + transferCharacteristics (u8) + matrixCoefficients (u8) + codecIntializationDataSize (u16)
  - 可変部: codecIntializationData (codecIntializationDataSize バイト)
  - 全フィールドを構造体メンバーとして保持する。reserved ビットは存在しない。bitDepth (4bit) / chromaSubsampling (3bit) / videoFullRangeFlag (1bit) は 1 バイトにパックされる。各フィールドは `u8` で保持し、`videoFullRangeFlag` のみ `bool` とする。`to_bytes` でビット幅を超えた値はマスクして書き出す (`AvcSequenceHeader::to_bytes` の `& 0x03` 慣例と同じ。round-trip は範囲内値でのみ成立する)
  - 検証: 固定部が 8 バイト未満は `Error::invalid_data`。codecIntializationDataSize に対してデータが不足している場合は `Error::invalid_data`。codecIntializationDataSize = 0 (空の初期化データ) は許容する (VP9 profile 0 では初期化データ不要)。`from_bytes` はパース後の末尾残余バイトを許容しない (固定部 + codecIntializationDataSize バイトで構造が確定するため、残余があれば不正入力として拒否する)
  - `to_bytes` では codecIntializationDataSize と実際のデータ長の一致を検証する
  - VP8 (0007) も同一の VPCodecConfigurationRecord を使用する (仕様 1322-1325 行)。型の共有/分離は 0007 側で判断する
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 1212-1215 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる
- legacy VideoTagHeader で VP9 を送る非標準実装はスコープ外とする。本 issue が対象とするのは eRTMP の `vp09` のみ

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `vp09` の VideoTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / SequenceEnd)
- `vp09` + CodedFramesX のデコード/エンコードが `Error::invalid_data` になる
- `Vp9SequenceHeader::from_bytes` / `to_bytes` が round-trip し、不正入力 (短すぎるデータ、codecIntializationDataSize に対して不足するデータ、末尾残余バイト) がエラーになる
- ユニットテストで上記の各ケースが確認できる
- pbt: VP9 用の Generator (`Vp9SequenceHeader` / VP9 を含む `VideoFrame`) が追加され、round-trip プロパティが通る
- fuzz: `Vp9SequenceHeader::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_vp9_sequence_header.rs`、`fuzz/Cargo.toml` への `[[bin]]` 追加を含む) が追加されている。`VideoFrame` デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、VP9 のコーパス種を `fuzz/corpus/` に追加する
- 既存の AVC (legacy / `avc1`) と他 legacy コーデックの動作に回帰が無い (既存テストがすべて通る)
- examples/server のシーケンスヘッダーキャッシュが VP9 の SequenceStart にも対応する (FFmpeg または OBS による手動確認でよい)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/media.rs`
   - `Vp9SequenceHeader` (VPCodecConfigurationRecord) を新規構造体として追加し、設計方針に記載したレイアウトで `from_bytes` / `to_bytes` を実装する
   - 0018 で追加済みの `VideoCodec::ExVp9` variant を利用して VP9 ペイロード処理を実装する (variant 自体の追加は 0018 のスコープ)
2. `src/flv.rs`
   - 0018 の FourCC 分岐で `vp09` を VP9 経路 (設計方針の各 VideoPacketType 処理) につなぐ
   - `vp09` + CodedFramesX を `Error::invalid_data` で拒否する分岐を追加する
3. `src/lib.rs`
   - `Vp9SequenceHeader` を re-export する
4. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲット・コーパスを追加する
5. `examples/server`
   - シーケンスヘッダーのキャッシュ判定を VP9 の SequenceStart にも対応させる。legacy AVC の SequenceHeader 判定も維持する
6. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
