# Enhanced RTMP の Opus 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-opus
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Audio 仕様のうち FourCC `Opus` に対応し、Opus を含む音声フレームを送受信できるようにする。

## 優先度根拠

High。Opus は時雨堂製品 (Sora 等) で広く採用されている音声コーデックであり、eRTMP 経由で Opus を扱えることは本ライブラリの主要ユースケースに直結する。
また、eRTMP v2 仕様で MultichannelConfig との連携を明示的に規定された数少ないコーデックの 1 つで、完全準拠の観点からも優先実装が必要。

## 現状

- ExAudioTagHeader 基盤 (SoundFormat::ExHeader 分岐、`AudioPacketType`、`AudioFourCc`、`MultichannelConfig`、`AudioFrame` のデータモデル再構成) は issue 0009 で導入される。本 issue はその基盤の上に `Opus` のペイロード処理を追加するスコープであり、0009 の完了が前提となる
- 0009 完了時点では、`Opus` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を Opus 実装で置き換える
- Opus 用シーケンスヘッダー (OpusSequenceHeader / ID header) に対応する型は存在しない (`src/media.rs:145`)

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`refs/enhanced-rtmp-v2.md` の 735 行以降) に準拠する
- 0009 で整備される ExAudioTagHeader 基盤の上に、FourCC が `Opus` (仕様 858 行) の場合の分岐とペイロード処理を追加する
- 各 AudioPacketType の Opus ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは OpusSequenceHeader (仕様 1009-1021 行。RFC 7845 Section 5.1 の ID header)。ペイロードが空の場合も許容する (仕様 1017-1019 行「If the Opus sequence start payload is empty, use the AudioPacketType.MultichannelConfig signal for channel mapping when present; otherwise, default to mono/stereo mode」)
  - `CodedFrames`: ボディは N ストリーム分の Opus パケット列 (仕様 1031-1047 行)。最初の (N-1) パケットは RFC 6716 Appendix B の self-delimiting framing、最終パケットは RFC 6716 Section 3 の undelimited framing。仕様原文は「Ogg packet」と記載しているが RTMP の AudioTag body を指す。全パケットは同一 duration でなければならない (仕様 1044-1045 行 MUST)。本ライブラリでは duration の検証は行わず (TOC バイトの解釈が追加で必要になるため)、パース結果は raw バイトを `AudioFrame::data` にそのまま格納する。パケット境界の解釈 (self-delimiting / undelimited framing による分割) は上位レイヤーの責務とする。エンコード時も `data` をそのまま書き出す
  - `SequenceEnd`: ボディ無し (仕様 984-986 行。SequenceEnd はコーデック非依存)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0009 の mp4a 分岐と同じ方針)
  - 上記以外の AudioPacketType (MultichannelConfig / Multitrack / ModEx) の扱いは 0009 のエラー分類に従う (MultichannelConfig は 0009 で実装済み、Multitrack は 0014、ModEx は 0015)
- OpusSequenceHeader (RFC 7845 Section 5.1 の ID header) を `OpusSequenceHeader` として表現し、`from_bytes` / `to_bytes` を提供する。RFC 7845 は `refs/` に存在しないため、以下のレイアウトを照合先とする:
  - 固定部 (19 バイト): magic_signature ([u8; 8]、= b"OpusHead")、version (u8、= 1)、output_channel_count (u8)、pre_skip (u16 LE)、input_sample_rate (u32 LE)、output_gain (i16 LE)、mapping_family (u8)
  - 条件部 (mapping_family != 0 の場合): stream_count (u8)、coupled_count (u8)、channel_mapping ([u8; output_channel_count])
  - 全フィールドを構造体メンバーとして保持する。`mapping_family` は `u8`、条件部は `Option<OpusChannelMapping>` で表現する
  - 検証: magic_signature != b"OpusHead" は `Error::invalid_data`。version != 1 は `Error::unsupported` (将来バージョンの前方互換)。output_channel_count = 0 は `Error::invalid_data`。mapping_family != 0 時に stream_count = 0 は `Error::invalid_data`。mapping_family != 0 時に coupled_count > stream_count は `Error::invalid_data` (RFC 7845 Section 5.1.1)。固定部が 19 バイト未満は `Error::invalid_data`。条件部が必要な場合にバイト不足は `Error::invalid_data`
  - `from_bytes` はパース後の末尾残余バイトを許容しない (ID header は一意に長さが確定する構造のため)
  - `to_bytes` では magic_signature / version の固定値検証と output_channel_count / stream_count の空チェックを行う。mapping_family != 0 かつ `OpusChannelMapping` が None (またはその逆) の構造的不整合は `Error::invalid_data` とする。リトルエンディアンで書き出す (RFC 7845 の規定どおり)
- ストリーム数 N の保持: N は mapping_family != 0 の場合は ID header の stream_count から、mapping_family = 0 の場合は無条件に 1 として導出する (output_channel_count によらず。RFC 7845 Section 5.1.1: mapping family 0 は単一ストリームに 1 または 2 チャンネルを格納)。SequenceStart / MultichannelConfig の双方が未受信で CodedFrames が到着した場合は N = 1 (mono/stereo フォールバック、仕様 1019 行) とする。N の参照元は `AudioFrame` に保持された `OpusSequenceHeader` または `MultichannelConfig` である (0009 のデータモデルに準拠)。本ライブラリでは CodedFrames のパケット分割は行わず、raw バイトを `AudioFrame::data` にそのまま格納する。パケット境界の解釈 (self-delimiting / undelimited framing) は上位レイヤーの責務とする
- エラー分類: 0009 の方針を継承する。Opus 固有の追加分類は上記の検証セクションのとおり
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 848-850 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `Opus` の AudioTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / SequenceEnd)
- `OpusSequenceHeader::from_bytes` / `to_bytes` が round-trip し、不正入力 (短すぎるデータ、不正な magic_signature、不正な version、output_channel_count=0、stream_count=0、coupled_count > stream_count、バイト不足) がエラーになる
- 空ペイロードの SequenceStart が許容される (MultichannelConfig 連携、または mono/stereo フォールバック)
- CodedFrames のデコード/エンコードが raw バイトの round-trip で正しく動作する (パケット分割は行わず、`AudioFrame::data` にそのまま格納)
- ユニットテストで上記の各ケースが確認できる
- pbt: Opus 用の Generator (`OpusSequenceHeader` / Opus を含む `AudioFrame`) が追加され、round-trip プロパティが通る
- fuzz: `OpusSequenceHeader::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_opus_sequence_header.rs`、`fuzz/Cargo.toml` への `[[bin]]` 追加を含む) が追加されている。AudioFrame デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、Opus のコーパス種を `fuzz/corpus/` に追加する
- 既存の legacy / 0009 で導入された他コーデックの動作に回帰が無い (既存テストがすべて通る)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/media.rs`
   - `OpusSequenceHeader` (RFC 7845 ID header) を新規構造体として追加し、設計方針に記載したレイアウトで `from_bytes` / `to_bytes` を実装する
   - 0009 で追加済みの `AudioFourCc::Opus` variant を利用して Opus ペイロード処理を実装する (variant 自体の追加は 0009 のスコープ)
2. `src/flv.rs`
   - 0009 の FourCC 分岐で `Opus` を Opus 経路 (設計方針の各 AudioPacketType 処理) につなぐ
3. `src/lib.rs`
   - `OpusSequenceHeader` を re-export する
4. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲット・コーパスを追加する
5. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
