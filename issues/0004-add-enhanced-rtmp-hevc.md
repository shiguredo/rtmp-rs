# Enhanced RTMP の HEVC (H.265) 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-hevc
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Video 仕様 (FourCC `hvc1`) に対応し、HEVC (H.265) を含む映像フレームを送受信できるようにする。
これにより、Sora など時雨堂製品との連携や、OBS Studio 30 以降・FFmpeg など eRTMP を実装している主要配信ツールとの相互接続が可能になる。

## 優先度根拠

High。Sora など時雨堂製品が RTMP 経由で HEVC 配信を取り扱う要件があり、本ライブラリが対応しないとプロダクト側で代替実装が必要になる。
配信側 (OBS Studio 30 以降、FFmpeg など) では eRTMP 実装が既に普及しており、相互接続の互換性確保が遅れるとプロダクト側にしわ寄せが出る。

## 現状

- `VideoCodec` enum は legacy FLV の `VideoCodecId` のみ定義しており、`Avc` までしか対応していない (`src/media.rs:120`)
- `decode_video_frame` / `encode_video_frame` (`src/flv.rs:96`, `src/flv.rs:111`) は VideoTagHeader の最上位ビット (IsExVideoHeader) を見ておらず、常に legacy フォーマットとして解釈する
- AVC 用シーケンスヘッダーは `AvcSequenceHeader` (`src/media.rs:226`) として AVCDecoderConfigurationRecord のパース/シリアライズが整備されているが、HEVC 用の対応する型は存在しない
- ExVideoTagHeader 周辺の `VideoPacketType` (SequenceStart, CodedFrames, CodedFramesX, SequenceEnd, Metadata, MPEG2TSSequenceStart, Multitrack, ModEx) や `VideoFourCc` の概念がコードベースに存在しない
- HEVC を受信した時点で、現状のデコーダは VideoCodecId として未知の値を読み取り `Invalid video codec` で失敗する (`src/flv.rs:143`)

## 設計方針

- Enhanced RTMP v2 の Enhanced Video セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 1086 行以降) に準拠する。v1 の `Defining Additional Video Codecs` も併せて参照する
- 本 issue では HEVC 必須最小機能のみをスコープに含める。以下は明示的にスコープ外とし、必要になった時点で別 issue として起票する
  - AV1 / VP9 / VP8 / VVC コーデック
  - Multitrack (`AvMultitrackType`)
  - ModEx (TimestampOffsetNano など)
  - Metadata Frame (colorInfo / HDR)
  - MPEG2TSSequenceStart
  - Enhanced Audio (FourCC ベース音声)
  - Reconnect Request
  - onMetaData の `videocodecid` リスト化など、connect コマンド側の eRTMP 拡張
- 既存の AVC 経路 (legacy VideoTagHeader) は維持する。IsExVideoHeader=0 のフレームは現在と同じ経路で処理する
- IsExVideoHeader=1 のフレームは、先頭バイトを `IsExVideoHeader(1) | VideoFrameType(3) | VideoPacketType(4)` として解釈し、続く FourCC が `hvc1` の場合に HEVC 経路へ分岐する
- `VideoPacketType` / `VideoFourCc` は本実装に必要な値だけ enum 化する (現時点では HEVC 関連のみ)。未対応値を受信した場合は仕様の MUST 要件 (eRTMP v2 1090 行) どおり「controlled and predictable manner で fail する」ため `Error::unsupported` を返す
- HEVCDecoderConfigurationRecord (ISO/IEC 14496-15:2022, 8.3.3.2) を `HevcSequenceHeader` として表現し、`AvcSequenceHeader` と同じ粒度で `from_bytes` / `to_bytes` を提供する
- `VideoFrame` 構造体への HEVC 情報の持たせ方は、`avc_packet_type` を一般化するか HEVC 用フィールドを追加するかを実装段階で決定する。判断軸は (a) 既存の公開 API 互換性 (b) `no_std` 制約 (c) HEVC 以外の今後の FourCC コーデック追加時の素直な拡張性、の 3 点
- `composition_timestamp_offset` の扱いは、HEVC の `CodedFrames` では SI24 を読み、`CodedFramesX` では暗黙 0 として扱う (仕様 1383-1423 行)

## 完了条件

- IsExVideoHeader=1 で VideoFourCc が `hvc1` の VideoTagHeader をデコード/エンコードできる
- VideoPacketType の `SequenceStart` / `CodedFrames` / `CodedFramesX` / `SequenceEnd` を HEVC ペイロードと正しく組み合わせて扱える
- HEVCDecoderConfigurationRecord のパース/シリアライズが round-trip する
- ユニットテストで以下が確認できる
  - HEVC の VideoTagHeader (各 VideoPacketType) の round-trip
  - `HevcSequenceHeader::from_bytes` / `to_bytes` の round-trip
  - 未対応の VideoPacketType (Metadata, MPEG2TSSequenceStart, Multitrack, ModEx) を受信した際に `ErrorKind::Unsupported` を返すこと
  - 未対応の VideoFourCc (`av01`, `vp09`, `vp08`, `vvc1` など) を受信した際に `ErrorKind::Unsupported` を返すこと
- pbt の HEVC 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
  - `HevcSequenceHeader::from_bytes`
  - ExVideoTagHeader を含む VideoFrame のデコード
- 既存の AVC / 他 legacy コーデックの動作に回帰が無い (既存テストがすべて通る)

## 解決方法

1. `src/media.rs`
   - `VideoCodec` enum に `Hevc` を追加する。legacy `VideoCodecId` と FourCC が衝突しない内部表現を採用する (legacy 用と FourCC 用を別 enum で持ち、`VideoCodec` をその統合 enum とする等)
   - HEVC ペイロード種別を表す型 (`HevcPacketType` または既存 `AvcPacketType` の一般化版) を VideoPacketType と対応付けて定義する
   - `HevcSequenceHeader` (HEVCDecoderConfigurationRecord) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する。`AvcSequenceHeader` の構造を参考にし、上限サイズ・上限個数の検証も同じ方針で揃える
2. `src/flv.rs`
   - `decode_video_frame` で先頭バイトの最上位ビットを判定し、ExVideoTagHeader 経路 (VideoPacketType + FourCC) と legacy 経路を分岐する
   - `encode_video_frame` で HEVC の場合は IsExVideoHeader=1、適切な VideoPacketType、FourCC `hvc1` を書き出す
   - VideoPacketType の `CodedFrames` では SI24 の compositionTimeOffset を読み書きし、`CodedFramesX` では暗黙 0 として扱う
3. `src/lib.rs`
   - `HevcSequenceHeader`、HEVC 関連の新規型を re-export する
4. `tests/`
   - HEVC の VideoTagHeader と `HevcSequenceHeader` の round-trip テストを追加する
   - 未対応の VideoPacketType / VideoFourCc を入力した際にエラーが返ることを確認するテストを追加する
5. `pbt/`
   - HEVC 用の Generator を追加する (`HevcSequenceHeader` / HEVC `VideoFrame`)
6. `fuzz/`
   - `HevcSequenceHeader::from_bytes` の fuzz ターゲットを追加する
   - ExVideoTagHeader を含む VideoFrame のデコード用 fuzz ターゲットを追加する
7. `examples/`
   - `publish` / `server` で HEVC の入出力に対応する必要がある場合はあわせて対応する (動作確認用、最低限の表示で良い)
