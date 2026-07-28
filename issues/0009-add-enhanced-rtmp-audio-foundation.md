# Enhanced RTMP の Enhanced Audio 基盤 (ExAudioTagHeader) を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-audio-foundation
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Audio 仕様が定義する ExAudioTagHeader (SoundFormat::ExHeader / AudioPacketType / AudioFourCc) のパース・シリアライズ基盤を追加する。
あわせて FourCC `mp4a` (ExAudioTagHeader 経由の AAC) と `.mp3` (ExAudioTagHeader 経由の MP3) を本基盤でサポートし、既存の AAC / MP3 ロジックを再利用して基盤単体で end-to-end に動作検証できるようにする。

本 issue は音声側の各コーデック issue (0010 Opus / 0011 FLAC / 0012 AC-3 / 0013 E-AC-3) がすべて依存する共通基盤である。映像側が 0018 (Enhanced Video 基盤) + 0004-0008 (各コーデック) と分離されているのと同じ粒度に揃える。

## 優先度根拠

High。0010-0013 の全音声コーデック issue が本 issue にブロックされる。配信側 (OBS Studio、FFmpeg 等) では eRTMP 実装が普及しており、相互接続の互換性確保の起点となる基盤である。

## 現状

- `AudioFormat` enum は legacy FLV の SoundFormat のみ定義しており、`DeviceSpecificSound = 15` まで対応している (`src/media.rs:145`)。ただし `LPcmPlatformEndian = 0` と `ExHeader = 9` は未定義 (12/13 は予約)
- `encode_audio_frame` / `decode_audio_frame` (`src/flv.rs:10`, `src/flv.rs:29`) は AudioTagHeader の先頭 4 ビット (SoundFormat) を見ておらず、SoundFormat=9 (`ExHeader`) の経路を持たない
- SoundFormat=9 のフレームを受信すると、`src/flv.rs:37` の match で `_ =>` に該当し `Invalid audio format: 9` (`ErrorKind::InvalidData`) で失敗する
- `AudioPacketType` / `AudioFourCc` / `AudioChannelOrder` / `AudioChannelMask` / `AudioChannel` / `MultichannelConfig` に相当する型はコードベースに存在しない
- `AudioFrame` (`src/media.rs:18`) は legacy 前提の構造 (`format: AudioFormat`、`is_aac_sequence_header: bool`) であり、ExAudioTagHeader の情報 (AudioPacketType、FourCC、MultichannelConfig) を保持できない

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`refs/enhanced-rtmp-v2.md` の 735 行以降) に準拠する
- 本 issue のスコープは「ExAudioTagHeader の枠組み + FourCC `mp4a` / `.mp3` + MultichannelConfig」のみとする。各コーデック固有の対応は依存 issue が扱う
  - `Opus` は 0010、`fLaC` は 0011、`ac-3` は 0012、`ec-3` は 0013
  - `Multitrack` (Audio) は 0014、`ModEx` (Audio) は 0015
  - connect コマンドでの FourCC signaling (仕様 848-850 行で MUST とされる) は 0017 で対応する。本 issue 完了時点ではデコード/エンコードのみ可能であり、配信ツールとの実際の相互接続には 0017 の完了が前提となる
- ワイヤフォーマット (仕様 749-887 行の擬似コードに準拠):
  - 先頭バイトの上位 4 ビットを SoundFormat として解釈し、`ExHeader = 9` の場合に ex 経路へ分岐する
  - ex 経路では下位 4 ビットを AudioPacketType として読み、ModEx ループ (仕様 784-821 行) を処理した後に FourCC を読む (MultichannelConfig パケットも仕様 835-837 行に従い FourCC を伴う)
  - ModEx ループは本 issue では `Error::unsupported` を返す枠のみ用意する (0015 で実装)
  - Multitrack (仕様 823-837 行) も本 issue では `Error::unsupported` を返す枠のみ用意する (0014 で実装)
- enum 定義 (数値はワイヤフォーマットそのもの。仕様 773-999 行):
  - `AudioPacketType`: SequenceStart=0, CodedFrames=1, SequenceEnd=2, MultichannelConfig=4, Multitrack=5, ModEx=7
  - `AudioFourCc`: Ac3=`ac-3`, Eac3=`ec-3`, Opus=`Opus`, Mp3=`.mp3`, Flac=`fLaC`, Aac=`mp4a`
  - `AudioChannelOrder`: Unspecified=0, Native=1, Custom=2 (仕様 893-915 行)
  - `AudioChannelMask`: FrontLeft=0x000001 〜 BottomFrontRight=0x800000 (仕様 917-952 行の全 24 値)
  - `AudioChannel`: FrontLeft=0 〜 BottomFrontRight=23, Unused=0xfe, Unknown=0xff (仕様 954-999 行)
- エラー分類 (仕様 739 行の MUST「fail in a controlled and predictable manner」への対応):
  - 認識できない値 (AudioPacketType 予約値 3/6/8-15、未知 FourCC、AudioChannelOrder 予約値 3 以上 (UI8 で読み取るため 16-255 も含む)、AudioChannel 予約値 24-0xfd) は `Error::invalid_data` (legacy 経路の未知値と同じ分類)
  - 認識できるが本 issue で未実装の値 (Multitrack / ModEx、および mp4a/.mp3 以外の既知 FourCC) は `Error::unsupported`。これは仕様 MUST の直接要求ではなく、「認識した上で未対応」を区別する本ライブラリの設計判断である
  - MultichannelConfig 内部の不正入力 (channelCount=0、バイト不足) は `Error::invalid_data` とする
- `mp4a` (AAC) のペイロード処理:
  - `SequenceStart`: ボディは AacSequenceHeader (仕様 989-993 行。ISO/IEC 14496-3 の AudioSpecificConfig)。既存の AAC シーケンスヘッダー解釈ロジックを再利用し、`AudioFrame::data` に生バイトで格納する
  - `CodedFrames`: ボディは AacCodedData (仕様 1056-1059 行)。既存の AAC raw データ解釈を再利用する
  - `SequenceEnd`: ボディ無し (仕様 984-986 行。SequenceEnd はコーデック非依存であり FourCC 分岐を持たない)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0018 の avc1 分岐と同じ方針)
- `.mp3` (MP3) のペイロード処理:
  - `CodedFrames`: ボディは Mp3CodedData (仕様 1049-1054 行)。既存の MP3 データ解釈を再利用する
  - `SequenceEnd`: ボディ無し (仕様 984-986 行。SequenceEnd はコーデック非依存であり FourCC 分岐を持たないため、`.mp3` でも有効)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする
  - `SequenceStart`: MP3 には仕様上定義されていない (仕様 988-1022 行の SequenceStart セクションに MP3 エントリなし)。`.mp3` + SequenceStart を受信した場合は `Error::invalid_data` とする
- MultichannelConfig (仕様 954-982 行):
  - `audioChannelOrder` (UI8) + `channelCount` (UI8) + 条件付きフィールド
  - `AudioChannelOrder::Custom` の場合: `audioChannelMapping` (UI8[channelCount]、各要素は AudioChannel)
  - `AudioChannelOrder::Native` の場合: `audioChannelFlags` (UI32、AudioChannelMask のビットマスク)
  - `AudioChannelOrder::Unspecified` の場合: 追加フィールドなし
  - `MultichannelConfig` 構造体として `from_bytes` / `to_bytes` を提供する
- `AudioFrame` のデータモデル:
  - `AudioFormat` は legacy SoundFormat (AdPcm=1 〜 Native=15、ただし LPcmPlatformEndian=0 と ExHeader=9 は除く既存の値域) と FourCC 系 (ExAac / ExMp3 / ExOpus 等) の統合 enum に再構成する。LPcmPlatformEndian=0 は現状 `src/flv.rs:50-55` で拒否されており、本 issue でも新たに受理しない (legacy 経路の動作を変更しないため)。`src/flv.rs:12` の `(frame.format as u8) << 4` キャストは成立しなくなるため、legacy SoundFormat への変換は明示的メソッド (例: `legacy_sound_format() -> Option<u8>`) に置き換える
  - ex 経路でデコードしたフレームでは、legacy 専用フィールド (`sample_rate` / `is_8bit_sample` / `is_stereo`) はワイヤ上に存在しない (仕様 756-758 行「soundRate, soundSize and soundType bits are not interpreted」)。これらのフィールドは `Option` 化し、ex 経路では `None` とする。`is_aac_sequence_header` は廃止し、AudioPacketType フィールドで代替する (ExAac + SequenceStart が旧 `is_aac_sequence_header=true` に相当)
  - `AudioFrame` に ExAudioTagHeader の情報 (AudioPacketType 相当) を保持するフィールドを追加する。MultichannelConfig パケットは `AudioFrame` として表現せず、`decode_audio_frame` の返り値を `Result<AudioFrame, Error>` から `Result<DecodedAudio, Error>` (DecodedAudio は Frame(AudioFrame) / MultichannelConfig(MultichannelConfig) の enum) に変更するか、あるいは `AudioFrame` に `Option<MultichannelConfig>` フィールドを持たせる。いずれかの設計を実装時に確定させるが、round-trip のためにパース結果を保持する方針は不変
  - これは公開 API の破壊的変更である。影響箇所: `src/flv.rs:12` のキャスト、`pbt/tests/prop_flv.rs:11-24` / `pbt/tests/prop_media.rs:10-23` / `pbt/tests/prop_rtmp_message.rs:130-143` / `pbt/tests/prop_rtmp_server_connection.rs:27-57` / `pbt/tests/prop_rtmp_client_connection.rs:35-65` / `pbt/tests/prop_rtmp_connection.rs:58-88` の `arb_audio_format` / `arb_audio_frame`、`src/rtmp_connection.rs` / `src/rtmp_message.rs` 内の crate 内ユニットテストの AudioFrame 構築箇所、`examples/publish/src/main.rs:405-427` の `AudioFrame` フィールドリテラル構築。これらの修正も本 issue のスコープに含める
  - `encode_audio_frame` は現状 `()` を返すが、ex 経路の検証 (`.mp3` + SequenceStart の拒否等) のため `Result<(), Error>` にシグネチャを変更する。legacy 経路は `Ok(())` を返すのみで動作不変
- 単一 audio message に複数の AudioPacketType が入り得るという Important 注記 (仕様 741 行「the bitstream MUST be processed completely」) は、主に Multitrack のバッチを想定したものである。本基盤では非 Multitrack の単一パケットのみを扱い、バッチ処理の設計は 0014 に委ねる。この判断を実装コメントに残す
- 既存の legacy 経路 (SoundFormat != ExHeader) の動作は一切変更しない。SoundFormat=0 (LPcmPlatformEndian) は現状どおり `Invalid audio format` で拒否し続ける (新たに variant を追加しない)

## 完了条件

- SoundFormat=9 (`ExHeader`) の AudioTagHeader をデコード/エンコードできる
- AudioPacketType の `SequenceStart` / `CodedFrames` / `SequenceEnd` / `MultichannelConfig` を扱える
- AudioFourCc の全列挙値 (Ac3=`ac-3` / Eac3=`ec-3` / Opus=`Opus` / Mp3=`.mp3` / Flac=`fLaC` / Aac=`mp4a`) を識別できる
- FourCC `mp4a` (AAC) の SequenceStart / CodedFrames / SequenceEnd が既存ロジック再利用で動作する
- FourCC `.mp3` (MP3) の CodedFrames / SequenceEnd が既存ロジック再利用で動作する。`.mp3` + SequenceStart は `Error::invalid_data` になる
- MultichannelConfig のパース/シリアライズが round-trip する (Unspecified / Native / Custom の各 ChannelOrder で確認)
- エラー分類が仕様どおり動作する
  - AudioPacketType 予約値 (3/6/8-15)、未知 FourCC → `ErrorKind::InvalidData`
  - Multitrack / ModEx、および `Opus` / `fLaC` / `ac-3` / `ec-3` → `ErrorKind::Unsupported`
- 既存の legacy 経路 (SoundFormat != ExHeader) に回帰が無い (既存テストの修正は `AudioFormat` 再構成に伴うコンパイル修正のみ)
- ユニットテストで上記の各ケースが確認できる
- pbt: `pbt/tests/prop_flv.rs` の `arb_audio_frame` を ex 経路 (mp4a / .mp3) を含むように拡張し、round-trip プロパティが通る
- fuzz: 既存 `fuzz/fuzz_targets/fuzz_flv.rs` が `decode_audio_frame` を入力全域でカバーしているため、新規ターゲットは追加せず、ex 経路のコーパス種を `fuzz/corpus/` に追加する。`MultichannelConfig::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_multichannel_config.rs`) は新規追加する
- CHANGES.md に CHANGE (後方互換のない変更) として記載されている

## 解決方法

1. `src/media.rs`
   - `AudioPacketType` / `AudioFourCc` / `AudioChannelOrder` / `AudioChannelMask` / `AudioChannel` enum を定義する (数値・FourCC 値は設計方針のとおり)
   - `AudioFormat` を legacy + FourCC の統合 enum に再構成し、`legacy_sound_format()` 等の明示的変換メソッドを追加する
   - `MultichannelConfig` 構造体を定義し、`from_bytes` / `to_bytes` を実装する
   - `AudioFrame` に ExAudioTagHeader の情報 (AudioPacketType 相当) を保持するフィールドを追加する
2. `src/flv.rs`
   - `decode_audio_frame` で先頭バイトの上位 4 ビットを判定し、ex 経路 (ModEx ループ → Multitrack 分岐 → FourCC 分岐 → MultichannelConfig / mp4a / .mp3 ペイロード処理) と legacy 経路を分岐する
   - `encode_audio_frame` のシグネチャを `Result<(), Error>` に変更し、ex 経路の書き出し (SoundFormat=ExHeader、AudioPacketType、FourCC) と検証 (`.mp3` + SequenceStart の拒否等) を実装する
3. `src/lib.rs`
   - 新規型 (`AudioPacketType` / `AudioFourCc` / `AudioChannelOrder` / `AudioChannelMask` / `AudioChannel` / `MultichannelConfig`) を re-export する
4. `tests/` / `pbt/` / `fuzz/` / `examples/`
   - 完了条件に列挙したテスト・pbt 拡張・fuzz ターゲット・コーパス追加を行う
   - `AudioFormat` 再構成に伴うコンパイル修正 (tests / pbt / examples の `AudioFrame` 構築箇所) を行う
5. `CHANGES.md`
   - CHANGE として記載する (`shiguredo-changelog` スキル参照)
