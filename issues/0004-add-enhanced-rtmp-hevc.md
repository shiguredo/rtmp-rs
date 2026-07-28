# Enhanced RTMP の HEVC (H.265) 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-hevc
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Video 仕様のうち FourCC `hvc1` に対応し、HEVC (H.265) を含む映像フレームを送受信できるようにする。

## 優先度根拠

High。Sora など時雨堂製品が RTMP 経由で HEVC 配信を取り扱う要件があり、本ライブラリが対応しないとプロダクト側で代替実装が必要になる。
配信側 (OBS Studio、FFmpeg など) では eRTMP による HEVC 配信が既に普及しており、相互接続の互換性確保が遅れるとプロダクト側にしわ寄せが出る。

## 現状

- ExVideoTagHeader 基盤 (IsExVideoHeader 分岐、`VideoPacketType`、`VideoFourCc`、FourCC `avc1` 対応、`VideoFrame` のデータモデル再構成) は issue 0018 で導入される。本 issue はその基盤の上に `hvc1` のペイロード処理を追加するスコープであり、0018 の完了が前提となる
- 0018 完了時点では、`hvc1` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を HEVC 実装で置き換える
- HEVC 用シーケンスヘッダー (HEVCDecoderConfigurationRecord) に対応する型は存在しない。AVC 用には `AvcSequenceHeader` (`src/media.rs:226`) が AVCDecoderConfigurationRecord のパース/シリアライズを提供しており、本 issue はこれと同等の型を HEVC 用に追加する

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`refs/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する
- 0018 で整備される ExVideoTagHeader 基盤の上に、FourCC が `hvc1` (仕様 1222 行) の場合の分岐とペイロード処理を追加する
- 各 VideoPacketType の HEVC ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは HEVCDecoderConfigurationRecord (仕様 1344-1349 行。「See ISO/IEC 14496-15:2022, 8.3.3.2」)。compositionTimeOffset はワイヤ上に存在しない (legacy AVC の SequenceHeader には CompositionTime があるため差分に注意)
  - `CodedFrames`: SI24 の compositionTimeOffset を読み、続くボディは 1 つ以上の NALU (仕様 1392-1399 行)
  - `CodedFramesX`: compositionTimeOffset は暗黙 0 で SI24 はワイヤ上に存在しない (仕様 1411-1413 行、1420-1423 行)
  - `SequenceEnd`: ボディ無し (仕様 1317-1319 行)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0018 の avc1 分岐と同じ方針)
  - 上記以外の VideoPacketType (Metadata / MPEG2TSSequenceStart / Multitrack / ModEx) の扱いは 0018 のエラー分類に従う (それぞれ 0015 / 0005 / 0014 / 0015 で対応)
- HEVCDecoderConfigurationRecord を `HevcSequenceHeader` として表現し、`AvcSequenceHeader` と同様に `from_bytes` / `to_bytes` を提供する。ISO/IEC 14496-15:2022 は `refs/` に存在しないため、以下のレイアウト (8.3.3.2 の要約) を照合先とする
  - 固定部: configurationVersion (u8、=1)、general_profile_space (2bit) / general_tier_flag (1bit) / general_profile_idc (5bit)、general_profile_compatibility_flags (u32)、general_constraint_indicator_flags (48bit)、general_level_idc (u8)、reserved(4bit)+min_spatial_segmentation_idc (12bit)、reserved(6bit)+parallelismType (2bit)、reserved(6bit)+chromaFormat (2bit)、reserved(5bit)+bitDepthLumaMinus8 (3bit)、reserved(5bit)+bitDepthChromaMinus8 (3bit)、avgFrameRate (u16)、constantFrameRate (2bit) / numTemporalLayers (3bit) / temporalIdNested (1bit) / lengthSizeMinusOne (2bit)、numOfArrays (u8)
  - 配列部: numOfArrays 個の { array_completeness (1bit) + reserved (1bit) + NAL_unit_type (6bit)、numNalus (u16)、numNalus 個の { nalUnitLength (u16) + NALU } }
   - 全フィールドを構造体メンバーとして保持する。固定部の reserved ビットは all 1s で書き出す (`AvcSequenceHeader::to_bytes` の `0xFC` 慣例と同じ)。配列部の reserved (1bit) は 0 固定で書き出す (ISO/IEC 14496-15:2022 8.3.3.2 の `unsigned int(1) reserved = 0`)。`array_completeness` も round-trip のため保持する。`general_constraint_indicator_flags` (48bit) は `u64` で保持し、上位 16bit は 0 固定とする
   - 検証: numOfArrays / numNalus はビット幅の上限 (255 / 65535) をそのまま許容する。`numOfArrays = 0` はエラーとする (VPS/SPS/PPS なしの HEVCDecoderConfigurationRecord は実用上意味をなさないため。`AvcSequenceHeader` の SPS/PPS 空リストエラーと同じ方針)。配列内の `numNalus = 0` もエラーとする。NALU 1 つあたりのサイズ上限は `AvcSequenceHeader` の独自上限と同じ 4096 バイトとする。`to_bytes` では空配列 (numOfArrays=0 / numNalus=0)、配列数上限 (255)・各配列の NALU 数上限 (65535)、NALU サイズ上限 (4096) を検証する (`AvcSequenceHeader::to_bytes` の SPS/PPS 個数検証 `src/media.rs:441-455` と同じ方針。pub Vec フィールドの長さはビット幅を超過し得るため)
   - `configurationVersion != 1` は `Error::unsupported` とする (`AvcSequenceHeader::from_bytes` の `src/media.rs:288` と同じ分類)
   - `from_bytes` はパース後の末尾残余バイトを黙認する (`AvcSequenceHeader::from_bytes` と同じ挙動。ISO 14496-15 の拡張フィールドが末尾に付く実装が存在するため)
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 1212-1215 行) であり、0017 で対応する。OBS 等の配信ツールは connect 応答の capability を見て HEVC を送るため、実際の相互接続には 0017 の完了が前提となる
  - OBS は HDR 付き HEVC 配信で `VideoPacketType.Metadata` (colorInfo) を送る。0015 完了までは HDR メタデータ付きストリームの Metadata パケットが `ErrorKind::Unsupported` になるため、HDR 配信の相互接続には 0015 の完了も必要となる
- legacy VideoTagHeader の VideoCodecId=12 で HEVC を送る非標準実装 (一部エンコーダの方言) はスコープ外とし、現状どおり `Invalid video codec` エラーのままとする。本 issue が対象とするのは eRTMP の `hvc1` のみ

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `hvc1` の VideoTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / CodedFramesX / SequenceEnd)
- `CodedFrames` の SI24 compositionTimeOffset と `CodedFramesX` の暗黙 0 が round-trip で区別して保持される (0018 のデータモデルに準拠)
- `HevcSequenceHeader::from_bytes` / `to_bytes` が round-trip し、不正入力 (短すぎるデータ、不正な configurationVersion、NALU サイズ上限超過、numNalus に対して不足するデータ、numOfArrays=0、numNalus=0、配列数上限超過 (to_bytes)、NALU 数上限超過 (to_bytes)) がエラーになる
- ユニットテストで上記の各ケースが確認できる
- pbt: HEVC 用の Generator (`HevcSequenceHeader` / HEVC を含む `VideoFrame`) が追加され、round-trip プロパティが通る
- fuzz: `HevcSequenceHeader::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_hevc_sequence_header.rs`、`fuzz/Cargo.toml` への `[[bin]]` 追加を含む) が追加されている。`VideoFrame` デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、HEVC のコーパス種を `fuzz/corpus/` に追加する
- 既存の AVC (legacy / `avc1`) と他 legacy コーデックの動作に回帰が無い (既存テストがすべて通る)
- examples/server のシーケンスヘッダーキャッシュが VideoPacketType::SequenceStart ベースで動作し、HEVC 中継が成立する (FFmpeg による手動確認でよい。OBS は 0017 完了後に確認可能)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/media.rs`
   - `HevcSequenceHeader` を新規構造体として追加し、設計方針に記載したレイアウトで `from_bytes` / `to_bytes` を実装する
   - 0018 で追加済みの `VideoCodec::ExHevc` variant を利用して HEVC ペイロード処理を実装する (variant 自体の追加は 0018 のスコープ)
2. `src/flv.rs`
   - 0018 の FourCC 分岐で `hvc1` を HEVC 経路 (設計方針の各 VideoPacketType 処理) につなぐ
3. `src/lib.rs`
   - `HevcSequenceHeader` を re-export する
4. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲット・コーパスを追加する
5. `examples/server`
   - シーケンスヘッダーのキャッシュ判定を 0018 のデータモデル (VideoPacketType::SequenceStart) にあわせて対応させ、HEVC のシーケンスヘッダーもキャッシュ対象に含める。legacy AVC の SequenceHeader 判定も維持する。HEVC 中継と legacy AVC 中継の両方が動作することを確認する。`examples/publish` は AVC 固定のため変更しない
6. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
