# Enhanced RTMP の Metadata Frame (colorInfo/HDR) と ModEx (TimestampOffsetNano) 対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-metadata-modex
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Metadata Frame 仕様 (`VideoPacketType.Metadata` + `colorInfo` HDR メタデータ) と、ExVideoTagHeader / ExAudioTagHeader 内の ModEx (TimestampOffsetNano によるナノ秒精度の timestamp 補正) に対応する。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義される機能であり、完全準拠の観点から実装は必要である。
仕様の `CapsExMask.ModEx` / `CapsExMask.TimestampNanoOffset` フラグでクライアント能力として明示的に宣言される。
時雨堂製品での直接的な需要は現時点で限定的なため Medium とする。

## 現状

- ExVideoTagHeader 基盤 (0018) と ExAudioTagHeader 基盤 (0009) は未完了であり、`VideoPacketType` / `AudioPacketType` 型はコードベースに存在しない。本 issue は 0018 と 0009 の完了が前提となる
- 0018 / 0009 完了時点では、`VideoPacketType.Metadata` / `VideoPacketType.ModEx` / `AudioPacketType.ModEx` を受信すると `ErrorKind::Unsupported` が返る。本 issue はこれらの分岐を実装で置き換える
- `colorInfo` を表現するための型 (ColorInfo / ColorConfig / HdrCll / HdrMdcv) はコードベースに存在しない
- ナノ秒オフセットを格納するためのフィールドも VideoFrame / AudioFrame に存在しない

## 設計方針

- Enhanced RTMP v2 の Metadata Frame セクション (`refs/enhanced-rtmp-v2.md` の 1446 行以降) と ExVideoTagHeader / ExAudioTagHeader 内 ModEx 記述に準拠する
- 本 issue では以下の 2 機能をまとめてスコープに含める。両者は `decode_video_frame` / `decode_audio_frame` 内の ExVideoTagHeader / ExAudioTagHeader 処理フローの一部であり、実装上同じ箇所を触るため 1 issue で扱う:
  - `VideoPacketType.Metadata` の AMF-encoded colorInfo パース/シリアライズ
  - ExVideoTagHeader / ExAudioTagHeader の ModEx ループと TimestampOffsetNano の扱い
- ワイヤフォーマット (仕様の該当行):
  - ModEx ループ (映像: 仕様 1167-1205 行、音声: 仕様 783-821 行):
    1. `modExDataSize = UI8 + 1` (1-256 バイト)
    2. `modExDataSize == 256` の場合: `modExDataSize = UI16 + 1` (257-65536 バイト)
    3. `modExData = UI8[modExDataSize]` を読む
    4. `videoPacketModExType = UB[4]` (modExData 直後の上位 4 bit)
    5. `videoPacketType = UB[4]` (下位 4 bit、バイト境界。ループ継続判定: これが ModEx ならループ継続)
    6. `TimestampOffsetNano` (type=0) の場合: `videoTimestampNanoOffset = bytesToUI24(modExData)` (先頭 3 バイトを UI24 として読む。仕様 1200 行)
  - Metadata ペイロード (仕様 1448 行): AMF-encoded の [name, value] ペア列。現在定義されているペアは `["colorInfo", Object]` のみ。AMF0 Object としてパースする (ECMA Array でも Object でも受信時に両方を許容。エンコード時は Object で書き出す)
- enum 定義 (数値はワイヤフォーマットそのもの):
  - `VideoPacketModExType`: TimestampOffsetNano=0 (仕様 1202 行)
  - `AudioPacketModExType`: TimestampOffsetNano=0 (仕様 802 行)
- ColorInfo 構造体 (仕様 1460-1543 行):
  - `colorConfig`: bitDepth (number, SHOULD be 8/10/12)、colorPrimaries (number, [0-255])、transferCharacteristics (number, [0-255])、matrixCoefficients (number, [0-255])
  - `hdrCll`: maxFall (number, [0.0001-10000])、maxCLL (number, [0.0001-10000])
  - `hdrMdcv`: redX/redY/greenX/greenY/blueX/blueY/whitePointX/whitePointY (number, 小数 4 桁)、maxLuminance (number, [5-10000])、minLuminance (number, [0.0001-5])
  - 全フィールドを `Option<f64>` で保持する (仕様の Note: 「Not all properties are guaranteed to be present」)
  - colorInfo リセット: `Undefined` 値 (推奨) または空オブジェクト `{}` (仕様 1448 行)。Rust では `Option<ColorInfo>` の `None` でリセットを表現する
- エラー分類:
  - `videoPacketModExType` / `audioPacketModExType` の予約値 (1-15): `Error::invalid_data`
  - `modExDataSize` に対して `modExData` が不足している場合: `Error::invalid_data`
  - Metadata ペイロードの AMF が不正な場合: `Error::invalid_data`
  - 未知の [name, value] ペアキーを受信した場合: 無視する (将来の拡張に備え、前方互換)
  - 0018 / 0009 のエラー分類を継承する
- データモデル:
  - VideoFrame に `timestamp_nano_offset: Option<u32>` フィールドを追加する (ModEx TimestampOffsetNano で取得した値。None はオフセットなし)
  - VideoFrame に `color_info: Option<ColorInfo>` フィールドを追加する (Metadata パケットで取得した値。None は未取得またはリセット済み)
  - AudioFrame に `timestamp_nano_offset: Option<u32>` フィールドを追加する
  - Metadata パケット (VideoPacketType.Metadata) は `decode_video_message` (0014 で追加) の返り値 `Vec<VideoFrame>` には含めず、colorInfo を直後のビデオフレームに適用する設計とする。ただし本 issue 完了時点では 0014 が未完了の場合があるため、`decode_video_frame` 単体では Metadata パケットを `Error::unsupported` のまま維持し、`decode_video_message` 内で処理する。0014 未完了の場合は Metadata パケットの colorInfo を `VideoFrame.color_info` に格納して返す暫定経路を用意する
- AMF0 のみ対応する。AMF3 encoding は connect コマンドでシグナリングされる (仕様 1457 行) が、その判定機構は 0017 のスコープである。0015 完了時点では AMF0 のみパースし、AMF3 マーカーを検出した場合は `Error::unsupported` を返す
- HDR formats (HDR10 / HDR10+ / HLG / DV) 自体の bitstream 解釈は本 issue のスコープ外 (codec bitstream 内の SEI/NALU/OBU は各コーデックの対応 issue で扱う)
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 1212-1215 行) であり、0017 で対応する

## 完了条件

- `VideoPacketType.Metadata` を含む video message をデコード/エンコードできる
- `ColorInfo` (colorConfig / hdrCll / hdrMdcv) を AMF0 で round-trip できる
- colorInfo の `Undefined` 値 / 空オブジェクトによるリセットを正しく扱える (`Option<ColorInfo>` の None として表現)
- ExVideoTagHeader / ExAudioTagHeader の ModEx ループを正しくパースできる (複数 ModEx パケット連続を含む)
- `TimestampOffsetNano` (UI24 ns 値) をパース/シリアライズできる
- `videoPacketModExType` / `audioPacketModExType` の予約値 (1-15) が `Error::invalid_data` になる
- 未知の [name, value] ペアキーを無視してパースが成功する
- ユニットテストで上記の各ケースが確認できる
- pbt: Metadata / ModEx 用 Generator が追加され、round-trip プロパティが通る
- fuzz: 既存 `fuzz/fuzz_targets/fuzz_flv.rs` が `decode_video_frame` / `decode_audio_frame` をカバーしているため、新規ターゲットは追加せず、Metadata / ModEx のコーパス種を `fuzz/corpus/` に追加する
- 既存の Enhanced Video / Enhanced Audio コーデック対応に回帰が無い (既存テストがすべて通る)
- CHANGES.md に CHANGE (VideoFrame / AudioFrame へのフィールド追加) + ADD (ColorInfo 型) として記載されている

## 解決方法

1. `src/media.rs`
   - `ColorInfo` / `ColorConfig` / `HdrCll` / `HdrMdcv` 構造体を定義する (全フィールド `Option<f64>`)
   - `VideoPacketModExType` / `AudioPacketModExType` enum を定義する (TimestampOffsetNano=0)
   - VideoFrame に `timestamp_nano_offset: Option<u32>` / `color_info: Option<ColorInfo>` フィールドを追加する
   - AudioFrame に `timestamp_nano_offset: Option<u32>` フィールドを追加する
2. `src/flv.rs`
   - ExVideoTagHeader / ExAudioTagHeader の ModEx ループを実装する (0018 / 0009 の `Error::unsupported` を置き換え)
   - VideoPacketType.Metadata の AMF-encoded colorInfo を読み書きする
3. `src/amf0.rs`
   - 既存の `Amf0Value` (Object / EcmaArray / Number / String / Undefined / Null) を利用して ColorInfo のシリアライズ/デシリアライズを実装する。新規 AMF0 型の追加は不要
4. `src/lib.rs`
   - `ColorInfo` / `ColorConfig` / `HdrCll` / `HdrMdcv` / `VideoPacketModExType` / `AudioPacketModExType` を re-export する
5. `tests/` / `pbt/` / `fuzz/` / `examples/`
   - 完了条件に列挙したテスト・Generator・コーパスを追加する
   - `timestamp_nano_offset` / `color_info` フィールド追加に伴うコンパイル修正 (VideoFrame / AudioFrame 構築箇所) を行う
6. `CHANGES.md`
   - CHANGE + ADD として記載する (`shiguredo-changelog` スキル参照)
