# Enhanced RTMP の Enhanced Audio 基盤 (ExAudioTagHeader) を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-audio-foundation
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Audio 仕様 (ExAudioTagHeader) の基盤を実装し、SoundFormat::ExHeader による FourCC 音声モードと、AudioPacketType / AudioFourCc / MultichannelConfig のデコード/エンコードを可能にする。
これにより、eRTMP 完全準拠を目指す本ライブラリの音声側の eRTMP 対応の起点とし、後続の個別コーデック (Opus / FLAC / AC-3 / E-AC-3) 対応 issue が乗る共通レイヤーを提供する。

## 優先度根拠

High。Enhanced Audio の全コーデック対応 issue (0010-0013) はすべてこの基盤に依存するため、最優先で着手する必要がある。
また、eRTMP 完全準拠を掲げる本ライブラリにとって、音声側の eRTMP 対応の起点として欠かせない実装である。

## 現状

- `decode_audio_frame` / `encode_audio_frame` (`src/flv.rs:10`, `src/flv.rs:29`) は legacy AudioTagHeader のみを扱い、SoundFormat の値が 9 (`ExHeader`) に該当する経路を持たない
- `AudioFormat` enum (`src/media.rs:144`) は legacy CodecID のみを定義しており、AudioFourCc / AudioPacketType / AudioChannelOrder / AudioChannelMask / AudioChannel の概念がコードベースに存在しない
- 既存の AAC / MP3 は legacy 経路でのみ扱えるが、eRTMP では FourCC `mp4a` / `.mp3` として再表現する経路も定義されている

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 735 行以降) に準拠する
- 本 issue では基盤のみをスコープに含め、以下は別 issue として起票する
  - 個別コーデックの SequenceStart / CodedFrames ペイロード処理 (Opus 0010、FLAC 0011、AC-3 0012、E-AC-3 0013)
  - Multitrack (Audio) は 0014
  - ModEx (Audio) は 0015
- 既存の legacy 経路 (SoundFormat != ExHeader) は維持する
- SoundFormat=9 (`ExHeader`) のフレームは、AudioPacketType と FourCC を読み取って分岐させる
- AudioPacketType のうち本基盤でハンドリングする値: `SequenceStart` (空ボディとして枠だけ用意)、`CodedFrames` (空ボディとして枠だけ用意)、`SequenceEnd`、`MultichannelConfig`
- 個別コーデックのペイロード解釈は別 issue で具体化するため、基盤側では「未対応コーデックを `Error::unsupported` で返す」枠組みを用意する
- AAC FourCC `mp4a` / MP3 FourCC `.mp3` は legacy 経路と並行する形で本基盤で受信可能にする (既存の AAC / MP3 ペイロード解釈ロジックを再利用)
- MultichannelConfig (`AudioChannelOrder` / `AudioChannelMask` / `AudioChannel` / channelCount / audioChannelMapping / audioChannelFlags) のパース・シリアライズを実装する
- 仕様 MUST 要件「未知の AudioPacketType / AudioFourCc に対して controlled and predictable manner で fail する」(eRTMP v2 739 行) を `Error::unsupported` で満たす

## 完了条件

- SoundFormat=9 (`ExHeader`) の AudioTagHeader をデコード/エンコードできる
- AudioPacketType の `SequenceStart` / `CodedFrames` / `SequenceEnd` / `MultichannelConfig` を扱える (個別コーデックペイロードは枠のみ)
- AudioFourCc の全列挙値 (Ac3 / Eac3 / Opus / Mp3 / Flac / Aac) を識別できる
- MultichannelConfig のパース/シリアライズが round-trip する (Unspecified / Native / Custom の各 ChannelOrder で確認)
- 既存の AAC / MP3 を FourCC 経路で扱える (legacy 経路と独立に、本基盤の枠組みで受信可能)
- ユニットテストで以下が確認できる
  - ExAudioTagHeader と MultichannelConfig の round-trip
  - 未対応の AudioPacketType (Multitrack / ModEx) を受信した際に `ErrorKind::Unsupported` を返すこと
  - 未対応の AudioFourCc を受信した際に `ErrorKind::Unsupported` を返すこと
- pbt の ExAudioTagHeader 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている (ExAudioTagHeader 全体のデコード)
- 既存の legacy AudioTagHeader 処理に回帰が無い

## 解決方法

1. `src/media.rs`
   - `AudioFormat` enum (またはこれを統合する新しい列挙) に FourCC モードを表現する手段を追加する。legacy `SoundFormat` と FourCC `AudioFourCc` を別 enum で持ち、`AudioFormat` をその統合 enum にする等
   - `AudioPacketType` (SequenceStart / CodedFrames / SequenceEnd / MultichannelConfig / Multitrack / ModEx) を定義する
   - `AudioFourCc` (Ac3 / Eac3 / Opus / Mp3 / Flac / Aac) を定義する
   - `AudioChannelOrder` / `AudioChannelMask` / `AudioChannel` を定義する
   - `MultichannelConfig` 構造体を定義し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - `decode_audio_frame` で先頭バイトの SoundFormat を判定し、`ExHeader` の場合に ExAudioTagHeader 経路へ分岐する
   - `encode_audio_frame` で FourCC モードを書き出す
   - ModEx ループは「Multitrack/ModEx は本 issue では未対応エラー」の枠だけ用意する
3. `src/lib.rs`
   - 新規型を re-export する
4. `tests/`
   - ExAudioTagHeader / MultichannelConfig の round-trip テストを追加する
   - 未対応値でのエラー返却を確認するテストを追加する
   - 既存の legacy AudioTagHeader テストを残し、回帰の無いことを確認する
5. `pbt/`
   - ExAudioTagHeader / MultichannelConfig 用 Generator を追加する
6. `fuzz/`
   - ExAudioTagHeader を含む AudioFrame デコードの fuzz ターゲットを追加する
   - `MultichannelConfig::from_bytes` の fuzz ターゲットを追加する
