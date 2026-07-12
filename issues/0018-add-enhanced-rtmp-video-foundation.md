# Enhanced RTMP の Enhanced Video 基盤 (ExVideoTagHeader) を追加する

- Priority: High
- Created: 2026-07-12
- Completed: {YYYY-MM-DD}
- Model: Fable 5
- Branch: feature/add-enhanced-rtmp-video-foundation
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP v2 の Enhanced Video 仕様が定義する ExVideoTagHeader (IsExVideoHeader / VideoPacketType / VideoFourCc) のパース・シリアライズ基盤を追加する。
あわせて FourCC `avc1` (ExVideoTagHeader 経由の AVC) を本基盤でサポートし、既存の AVC ロジックを再利用して基盤単体で end-to-end に動作検証できるようにする。

本 issue は映像側の各コーデック issue (0004 HEVC / 0005 AV1 / 0006 VP9 / 0007 VP8 / 0008 VVC) がすべて依存する共通基盤である。音声側が 0009 (Enhanced Audio 基盤) + 0010-0013 (各コーデック) と分離されているのと同じ粒度に揃える。

## 優先度根拠

High。0004-0008 の全映像コーデック issue が本 issue にブロックされる。配信側 (OBS Studio、FFmpeg 等) では eRTMP 実装が普及しており、相互接続の互換性確保の起点となる基盤である。

## 現状

- `VideoCodec` enum は legacy FLV の VideoCodecId のみ定義しており、`Avc = 7` までしか対応していない (`src/media.rs:120`)
- `encode_video_frame` / `decode_video_frame` (`src/flv.rs:96`, `src/flv.rs:111`) は VideoTagHeader の最上位ビット (IsExVideoHeader) を見ておらず、常に legacy フォーマットとして解釈する
- IsExVideoHeader=1 のフレームを受信すると、`src/flv.rs:118` の `(frame_type_codec >> 4) & 0x0F` が 8 以上になり、`src/flv.rs:126` の `Invalid video frame type` (`ErrorKind::InvalidData`) で失敗する (コーデック判定には到達しない)
- `VideoPacketType` / `VideoFourCc` / `VideoCommand` に相当する型はコードベースに存在しない
- `VideoFrame` (`src/media.rs:57`) は legacy 前提の構造 (`codec: VideoCodec`、`avc_packet_type: Option<AvcPacketType>`) であり、ExVideoTagHeader の情報 (VideoPacketType、FourCC) を保持できない

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`refs/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する
- 本 issue のスコープは「ExVideoTagHeader の枠組み + FourCC `avc1`」のみとする。各コーデック固有の対応は依存 issue が扱う
  - `hvc1` (HEVC) は 0004、`av01` (AV1) は 0005、`vp09` (VP9) は 0006、`vp08` (VP8) は 0007、`vvc1` (VVC) は 0008
  - `VideoPacketType.Metadata` / `ModEx` は 0015、`Multitrack` は 0014、`MPEG2TSSequenceStart` は 0005 (AV1 用) で対応する
  - connect コマンドでの FourCC signaling (`videoFourCcInfoMap` / `capsEx`。仕様 1212-1215 行で MUST とされる) は 0017 で対応する。本 issue 完了時点ではデコード/エンコードのみ可能であり、配信ツールとの実際の相互接続には 0017 の完了が前提となる
- ワイヤフォーマット (仕様 1094-1215 行の擬似コードに準拠):
  - 先頭バイトを `IsExVideoHeader(1) | VideoFrameType(3) | VideoPacketType(4)` として解釈する
  - `videoPacketType != Metadata` かつ `videoFrameType == Command` の場合、続くのは FourCC ではなく UI8 の VideoCommand であり、ExVideoTagBody は存在しない (仕様 1207-1215 行)。`VideoCommand` enum (StartSeek=0, EndSeek=1) を定義してパースする
  - 上記以外の場合、続く UI32 を FourCC として読み、`avc1` なら本基盤の AVC 経路へ、それ以外の既知 FourCC は `Error::unsupported` (依存 issue で置き換え)、未知 FourCC は `Error::invalid_data` とする
- enum 定義 (数値はワイヤフォーマットそのもの。仕様 1157-1200 行および 1218-1223 行):
  - `VideoPacketType`: SequenceStart=0, CodedFrames=1, SequenceEnd=2, CodedFramesX=3, Metadata=4, MPEG2TSSequenceStart=5, Multitrack=6, ModEx=7
  - `VideoFourCc`: Vp8=`vp08`, Vp9=`vp09`, Av1=`av01`, Avc=`avc1`, Hevc=`hvc1`, Vvc=`vvc1`
- エラー分類 (仕様 1090 行の MUST「fail in a controlled and predictable manner」への対応):
  - 認識できない値 (VideoFrameType 予約値 0/6/7、VideoPacketType 予約値 8-15、未知 FourCC) は `Error::invalid_data` (legacy 経路の未知値と同じ分類)
  - 認識できるが本 issue で未実装の値 (Metadata / MPEG2TSSequenceStart / Multitrack / ModEx、および avc1 以外の既知 FourCC) は `Error::unsupported`。これは仕様 MUST の直接要求ではなく、「認識した上で未対応」を区別する本ライブラリの設計判断である
- `avc1` のペイロード処理:
  - `SequenceStart`: ボディは AVCDecoderConfigurationRecord。既存 `AvcSequenceHeader` (`src/media.rs:226`) をそのまま利用し、`VideoFrame::data` に生バイトで格納する (legacy AVC と同じ慣行)
  - `CodedFrames`: SI24 の compositionTimeOffset を読み、続くボディは NALU 列 (仕様 1383-1390 行)
  - `CodedFramesX`: compositionTimeOffset は暗黙 0 (仕様 1411-1413 行。SI24 はワイヤ上に存在しない)
  - `SequenceEnd`: ボディ無し (仕様 1317-1319 行)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする
- `VideoFrame` のデータモデル:
  - `CodedFrames` (SI24 明示) と `CodedFramesX` (暗黙 0) は別のワイヤ表現であり、round-trip のために `VideoPacketType` 相当をフレームに保持する。compositionTimeOffset の値だけを持つ設計は不可
  - `VideoCodec` は legacy VideoCodecId (Jpeg=1 〜 Avc=7) と FourCC 系 (ExAvc / ExHevc 等、FourCC で表現されるコーデック) の統合 enum に再構成する。`src/flv.rs:98` の `frame.codec as u8` キャストは成立しなくなるため、legacy コーデック ID への変換は明示的メソッド (例: `legacy_codec_id() -> Option<u8>`) に置き換える
  - これは公開 API の破壊的変更である。影響箇所: `src/flv.rs:98` のキャスト、`pbt/tests/prop_media.rs:203-208` の値域検証、`tests/rtmp_publish_client_test.rs:162` / `tests/rtmp_play_client_test.rs:54, 191` / `examples/publish/src/main.rs:348-372` / `examples/server/src/main.rs` の `VideoFrame` フィールドリテラル構築。これらの修正も本 issue のスコープに含める
  - `encode_video_frame` が `avc_packet_type.is_some()` をトリガーに AVC 拡張部を書く一方、`decode_video_frame` は `codec == Avc && frame_type != VideoInfoOrCommandFrame` で分岐する非対称 (`src/flv.rs:102`, `src/flv.rs:153`) があるため、ex 経路の追加時に encode / decode の分岐条件を対で設計し round-trip を保証する
- 単一 video message に複数の VideoPacketType が入り得るという Important 注記 (仕様 1092 行「the bitstream MUST be processed completely」) は、主に Multitrack / Metadata のバッチを想定したものである。本基盤では非 Multitrack の単一パケットのみを扱い、バッチ処理の設計は 0014 / 0015 に委ねる。この判断を実装コメントに残す
- 既存の legacy 経路 (IsExVideoHeader=0) の動作は一切変更しない

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `avc1` の VideoTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / CodedFramesX / SequenceEnd)
- `CodedFrames` と `CodedFramesX` が round-trip で区別して保持される
- VideoFrameType.Command + VideoCommand (StartSeek / EndSeek) をパースできる
- エラー分類が仕様どおり動作する
  - VideoFrameType 予約値 (0/6/7)、VideoPacketType 予約値 (8-15)、未知 FourCC → `ErrorKind::InvalidData`
  - Metadata / MPEG2TSSequenceStart / Multitrack / ModEx、および `hvc1` / `av01` / `vp09` / `vp08` / `vvc1` → `ErrorKind::Unsupported`
- SequenceEnd に残余バイトがある入力がエラーになる
- 既存の legacy 経路 (IsExVideoHeader=0) に回帰が無い (既存テストの修正は `VideoCodec` 再構成に伴うコンパイル修正のみ)
- ユニットテストで上記の各ケースが確認できる
- pbt: `pbt/tests/prop_flv.rs` の `arb_video_frame` を ex 経路 (avc1) を含むように拡張し、round-trip プロパティが通る
- fuzz: 既存 `fuzz/fuzz_targets/fuzz_flv.rs` が `decode_video_frame` を入力全域でカバーしているため、新規ターゲットは追加せず、ex 経路のコーパス種を `fuzz/corpus/` に追加する
- CHANGES.md に CHANGE (後方互換のない変更) として記載されている

## 解決方法

1. `src/media.rs`
   - `VideoPacketType` / `VideoFourCc` / `VideoCommand` enum を定義する (数値・FourCC 値は設計方針のとおり)
   - `VideoCodec` を legacy + FourCC の統合 enum に再構成し、`legacy_codec_id()` 等の明示的変換メソッドを追加する
   - `VideoFrame` に ExVideoTagHeader の情報 (VideoPacketType 相当) を保持するフィールドを追加する
2. `src/flv.rs`
   - `decode_video_frame` で先頭バイトの bit 7 を判定し、ex 経路 (Command 分岐 → FourCC 分岐 → avc1 ペイロード処理) と legacy 経路を分岐する
   - `encode_video_frame` で ex 経路の書き出し (IsExVideoHeader=1、VideoPacketType、FourCC、CodedFrames の SI24) を実装する
3. `src/lib.rs`
   - 新規型 (`VideoPacketType` / `VideoFourCc` / `VideoCommand`) を re-export する
4. `tests/` / `pbt/` / `fuzz/` / `examples/`
   - 完了条件に列挙したテスト・pbt 拡張・コーパス追加を行う
   - `VideoCodec` 再構成に伴うコンパイル修正 (tests / pbt / examples の `VideoFrame` 構築箇所) を行う
5. `CHANGES.md`
   - CHANGE として記載する (`shiguredo-changelog` スキル参照)
