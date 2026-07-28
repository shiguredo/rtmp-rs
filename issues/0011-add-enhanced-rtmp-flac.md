# Enhanced RTMP の FLAC 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-flac
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Audio 仕様のうち FourCC `fLaC` に対応し、FLAC を含む音声フレームを送受信できるようにする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
FLAC はロスレス音声の代表として位置付けられるが、時雨堂製品での直接的な需要は現時点で限定的なため、優先度は Medium とする。

## 現状

- ExAudioTagHeader 基盤 (SoundFormat::ExHeader 分岐、`AudioPacketType`、`AudioFourCc`、`MultichannelConfig`、`AudioFrame` のデータモデル再構成) は issue 0009 で導入される。本 issue はその基盤の上に `fLaC` のペイロード処理を追加するスコープであり、0009 の完了が前提となる
- 0009 完了時点では、`fLaC` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を FLAC 実装で置き換える
- FLAC 用シーケンスヘッダー (FlacSequenceHeader) に対応する型は存在しない。AVC 用には `AvcSequenceHeader` (`src/media.rs:226`) が存在するが、FLAC 用はない

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`refs/enhanced-rtmp-v2.md` の 735 行以降) に準拠する
- 0009 で整備される ExAudioTagHeader 基盤の上に、FourCC が `fLaC` (仕様 864 行) の場合の分岐とペイロード処理を追加する
- 各 AudioPacketType の FLAC ペイロード (仕様の該当行):
  - `SequenceStart`: ボディは FlacSequenceHeader (仕様 995-1007 行)。FLAC `fLaC` シグネチャ (4 バイト: 0x66 0x4C 0x61 0x43) + STREAMINFO メタデータブロック (FLAC 仕様 Section 7 の STREAMINFO。ブロックヘッダ 4 バイト + ブロックデータ 34 バイト)
  - `CodedFrames`: ボディは 1 つ以上の FLAC 音声フレーム (仕様 1061-1068 行)。各フレームはフレームヘッダ (同期コード + ブロックサイズ + サンプルレート等の情報) で自己記述される。パース結果は raw バイトを `AudioFrame::data` にそのまま格納する。フレーム境界の解釈は上位レイヤーの責務とする。エンコード時も `data` をそのまま書き出す
  - `SequenceEnd`: ボディ無し (仕様 984-986 行。SequenceEnd はコーデック非依存)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0009 の mp4a 分岐と同じ方針)
  - 上記以外の AudioPacketType (MultichannelConfig / Multitrack / ModEx) の扱いは 0009 のエラー分類に従う (MultichannelConfig は 0009 で実装済み、Multitrack は 0014、ModEx は 0015)
- FlacSequenceHeader を `FlacSequenceHeader` として表現し、`from_bytes` / `to_bytes` を提供する。FLAC 仕様 (https://xiph.org/flac/format.html) は `refs/` に存在しないため、以下のレイアウトを照合先とする:
  - 構造: fLaC シグネチャ ([u8; 4]、= 0x66 0x4C 0x61 0x43) + メタデータブロックヘッダ (4 バイト: last-metadata-block flag (1bit) + block type (7bit) + block length (24bit)) + STREAMINFO ブロックデータ (34 バイト)
  - STREAMINFO ブロックデータ (34 バイト): min_block_size (u16 BE)、max_block_size (u16 BE)、min_frame_size (24bit BE)、max_frame_size (24bit BE)、sample_rate (20bit BE)、channels (3bit BE、値は channels-1)、bits_per_sample (5bit BE、値は bits-1)、total_samples (36bit BE)、md5_signature ([u8; 16])
  - 全フィールドを構造体メンバーとして保持する。ただし fLaC シグネチャ・メタデータブロックヘッダ (last-metadata-block flag / block type / block length) は固定値として扱い、構造体フィールドにしない (from_bytes で読み捨て、to_bytes で常に fLaC / last-metadata-block=true / block type=0 / block length=34 を書き出す。これにより round-trip の対称性を保証する)。channels は実チャンネル数 (u8、ワイヤ値 + 1)、bits_per_sample は実ビット数 (u8、ワイヤ値 + 1) として保持し、エンコード時に -1 して書き出す
  - 検証: fLaC シグネチャ不一致は `Error::invalid_data`。block type != 0 (STREAMINFO) は `Error::invalid_data`。block length != 34 は `Error::invalid_data`。min_block_size = 0 または max_block_size = 0 は `Error::invalid_data`。sample_rate = 0 は `Error::invalid_data`。最小バイト数 (4 + 4 + 34 = 42 バイト) 未満は `Error::invalid_data`
  - `from_bytes` はパース後の末尾残余バイトを許容する (FLAC 仕様では STREAMINFO 以降に追加のメタデータブロックが続く場合があるため。本ライブラリでは STREAMINFO のみパースし、残余は無視する)
  - `to_bytes` では min_block_size / max_block_size / sample_rate / channels / bits_per_sample の非ゼロ検証を行う (channels / bits_per_sample は 0 の場合エンコード時にアンダーフローするため)。channels は 1-8、bits_per_sample は 1-32 の範囲検証も行う (3bit / 5bit フィールドに収まること)。fLaC シグネチャ / last-metadata-block=true / block type=0 / block length=34 は固定値として書き出す (フィールドではないため検証不要)。ビッグエンディアンで書き出す (FLAC 仕様の規定どおり)
- エラー分類: 0009 の方針を継承する。FLAC 固有の追加分類は上記の検証セクションのとおり
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 848-850 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `fLaC` の AudioTagHeader をデコード/エンコードできる (SequenceStart / CodedFrames / SequenceEnd)
- `FlacSequenceHeader::from_bytes` / `to_bytes` が round-trip し、不正入力 (短すぎるデータ、不正な fLaC シグネチャ、不正な block type、不正な block length、min_block_size=0、max_block_size=0、sample_rate=0、channels=0、bits_per_sample=0、channels 範囲超過、bits_per_sample 範囲超過) がエラーになる (from_bytes / to_bytes 両方)
- CodedFrames のデコード/エンコードが raw バイトの round-trip で正しく動作する (フレーム分割は行わず、`AudioFrame::data` にそのまま格納)
- ユニットテストで上記の各ケースが確認できる
- pbt: FLAC 用の Generator (`FlacSequenceHeader` / FLAC を含む `AudioFrame`) が追加され、round-trip プロパティが通る
- fuzz: `FlacSequenceHeader::from_bytes` の fuzz ターゲット (`fuzz/fuzz_targets/fuzz_flac_sequence_header.rs`、`fuzz/Cargo.toml` への `[[bin]]` 追加を含む) が追加されている。AudioFrame デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、FLAC のコーパス種を `fuzz/corpus/` に追加する
- 既存の legacy / 0009 で導入された他コーデックの動作に回帰が無い (既存テストがすべて通る)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/media.rs`
   - `FlacSequenceHeader` (fLaC シグネチャ + STREAMINFO) を新規構造体として追加し、設計方針に記載したレイアウトで `from_bytes` / `to_bytes` を実装する
   - 0009 で追加済みの `AudioFourCc::Flac` variant を利用して FLAC ペイロード処理を実装する (variant 自体の追加は 0009 のスコープ)
2. `src/flv.rs`
   - 0009 の FourCC 分岐で `fLaC` を FLAC 経路 (設計方針の各 AudioPacketType 処理) につなぐ
3. `src/lib.rs`
   - `FlacSequenceHeader` を re-export する
4. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲット・コーパスを追加する
5. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
