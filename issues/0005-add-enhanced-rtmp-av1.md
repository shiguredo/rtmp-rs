# Enhanced RTMP の AV1 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-av1
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Video 仕様のうち FourCC `av01` に対応し、AV1 を含む映像フレームを送受信できるようにする。

## 優先度根拠

High。eRTMP v2 仕様で AV1 は中心的なコーデックの 1 つであり、本ライブラリが eRTMP 完全準拠を掲げる以上、優先実装すべき対象である。
配信側 (FFmpeg、OBS Studio 30 系等) で AV1 配信が普及しており、相互接続の互換性確保が必要。HEVC (issue 0004) と同等の優先度で扱う。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader 分岐、`VideoPacketType`、`VideoFourCc`、FourCC `avc1` 対応、`VideoFrame` のデータモデル再構成) は issue 0018 で導入される。本 issue はその基盤の上に `av01` のペイロード処理を追加するスコープであり、0018 の完了が前提となる
- 0018 完了時点では、`av01` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を AV1 実装で置き換える
- AV1 用シーケンスヘッダー (AV1CodecConfigurationRecord) に対応する型は存在しない。AVC 用には `AvcSequenceHeader` (`src/media.rs:226`) が AVCDecoderConfigurationRecord のパース/シリアライズを提供しており、本 issue はこれと同等の型を AV1 用に追加する
- AV1 は MPEG2TSSequenceStart にも対応する唯一のコーデックである (仕様 1360-1365 行)。SequenceStart と MPEG2TSSequenceStart の 2 系統を扱う必要がある

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`refs/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する
- 0018 で整備される ExVideoTagHeader 基盤の上に、FourCC が `av01` (仕様 1220 行) の場合の分岐とペイロード処理を追加する
- 各 VideoPacketType の AV1 ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは AV1CodecConfigurationRecord (仕様 1332-1335 行)
  - `MPEG2TSSequenceStart`: ボディは AV1VideoDescriptor (仕様 1360-1365 行)。SequenceStart と MPEG2TSSequenceStart は仕様上相互排他である (仕様 1179-1180 行「PacketTypeSequenceStart and PacketTypeMPEG2TSSequenceStart are mutually exclusive」)。ただし `decode_video_frame` はステートレス関数でありストリームレベルの混在は検知不可能なため、本 issue ではメッセージ単位のパースのみ行い、相互排他の強制は将来の接続状態管理レイヤーに委ねる (仕様上の制約として注記するのみ)
  - `CodedFrames`: ボディは 1 つ以上の OBU からなる単一 temporal unit (仕様 1378-1381 行)。AVC/HEVC/VVC と異なり、AV1 の CodedFrames には compositionTimeOffset (SI24) が存在しない。ワイヤ上から SI24 を読まず、直ちに OBU データが始まる。デコード時は `composition_timestamp_offset` に `RtmpTimestampDelta::ZERO` を設定し、エンコード時は同フィールドを無視する (pbt Generator も ZERO に拘束する)
  - `CodedFramesX`: AV1 には定義されていない (仕様 1411-1429 行の CodedFramesX セクションに AV1 エントリは存在しない)。`av01` + CodedFramesX を受信した場合は `Error::invalid_data` とする
  - `SequenceEnd`: ボディ無し (仕様 1317-1319 行)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0018 の avc1 分岐と同じ方針)
  - 上記以外の VideoPacketType (Metadata / Multitrack / ModEx) の扱いは 0018 のエラー分類に従う (それぞれ 0015 / 0014 / 0015 で対応)
- AV1CodecConfigurationRecord を `Av1SequenceHeader` として表現し、`AvcSequenceHeader` と同様に `from_bytes` / `to_bytes` を提供する。AV1-ISOBMFF specification (av1-isobmff v1.2.0, Section 2.3) は `refs/` に存在しないため、以下のレイアウトを照合先とする:
  - 固定部 (4 バイト): marker (1bit、=1) + version (7bit、=1) + seq_profile (3bit) + seq_level_idx_0 (5bit) + seq_tier_0 (1bit) + high_bitdepth (1bit) + twelve_bit (1bit) + monochrome (1bit) + chroma_subsampling_x (1bit) + chroma_subsampling_y (1bit) + chroma_sample_position (2bit) + reserved (3bit、=0) + initial_presentation_delay_present (1bit) + initial_presentation_delay_minus_one (4bit、present=1 の場合) または reserved (4bit、=0、present=0 の場合)
  - 可変部: configOBUs (残りの全バイト。OBU の配列)
  - 全フィールドを構造体メンバーとして保持する。reserved ビットは 0 固定で書き出す (AV1-ISOBMFF の規定どおり。HEVC の all 1s 慣例とは異なる)
  - 検証: marker != 1 または version != 1 は `Error::unsupported`。configOBUs が空 (0 バイト) は `Error::invalid_data` とする。`from_bytes` はパース後の末尾残余バイトを許容しない (configOBUs が残り全バイトを消費するため、残余は構造上発生しない)
  - `to_bytes` では marker/version の固定値検証と configOBUs 空チェックを行う。ビットフィールドメンバー (seq_profile 等) がビット幅を超えた場合はマスクして書き出す (`AvcSequenceHeader::to_bytes` の `& 0x03` 慣例と同じ。round-trip は範囲内値でのみ成立する)
  - `initial_presentation_delay_present` は `bool`、`initial_presentation_delay_minus_one` は `Option<u8>` で表現する (present=false の場合は None)。`to_bytes` は `present` フィールドを正として書き出す。`present=true` かつ `None` の場合は `Error::invalid_data`、`present=false` かつ `Some(_)` の場合は値を無視して reserved=0 を書き出す
- AV1VideoDescriptor は MPEG-2 TS の video_stream_descriptor に相当するが、eRTMP 仕様は内部構造を定義していない (仕様 1363 行 `av1Header = [AV1VideoDescriptor]` のみ)。本 issue では AV1VideoDescriptor を opaque なバイト列 (`Vec<u8>`) として扱い、パース/検証は行わない。空 (0 バイト) も許容する (SequenceEnd の「ボディ無し」とは異なり、descriptor の最小長は仕様未定義のため)。round-trip のみ保証する。将来、内部構造の解析が必要になったら別 issue で対応する
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 1212-1215 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる
- legacy VideoTagHeader で AV1 を送る非標準実装はスコープ外とする。本 issue が対象とするのは eRTMP の `av01` のみ

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `av01` の VideoTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / SequenceEnd / MPEG2TSSequenceStart)
- `av01` + CodedFramesX の入力が `Error::invalid_data` になる
- `Av1SequenceHeader::from_bytes` / `to_bytes` が round-trip し、不正入力 (短すぎるデータ、不正な marker/version、configOBUs 空) がエラーになる
- MPEG2TSSequenceStart の AV1VideoDescriptor が opaque バイト列として round-trip する
- ユニットテストで上記の各ケースが確認できる
- pbt: AV1 用の Generator (`Av1SequenceHeader` / AV1 を含む `VideoFrame`) が追加され、round-trip プロパティが通る
- fuzz: `Av1SequenceHeader::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_av1_sequence_header.rs`、`fuzz/Cargo.toml` への `[[bin]]` 追加を含む) が追加されている。`VideoFrame` デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、AV1 のコーパス種を `fuzz/corpus/` に追加する
- 既存の AVC (legacy / `avc1`) と他 legacy コーデックの動作に回帰が無い (既存テストがすべて通る)
- examples/server のシーケンスヘッダーキャッシュが AV1 の SequenceStart / MPEG2TSSequenceStart にも対応する (FFmpeg または OBS による手動確認でよい)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/media.rs`
   - `Av1SequenceHeader` (AV1CodecConfigurationRecord) を新規構造体として追加し、設計方針に記載したレイアウトで `from_bytes` / `to_bytes` を実装する
   - 0018 で追加済みの `VideoCodec::ExAv1` variant を利用して AV1 ペイロード処理を実装する (variant 自体の追加は 0018 のスコープ)
2. `src/flv.rs`
   - 0018 の FourCC 分岐で `av01` を AV1 経路 (設計方針の各 VideoPacketType 処理) につなぐ
   - `MPEG2TSSequenceStart` の AV1VideoDescriptor を opaque バイト列として読み書きする
   - `av01` + CodedFramesX を `Error::invalid_data` で拒否する分岐を追加する
3. `src/lib.rs`
   - `Av1SequenceHeader` を re-export する
4. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲット・コーパスを追加する
5. `examples/server`
   - シーケンスヘッダーのキャッシュ判定を AV1 の SequenceStart / MPEG2TSSequenceStart にも対応させる。legacy AVC の SequenceHeader 判定も維持する
6. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
