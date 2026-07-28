# Enhanced RTMP の AC-3 配信/受信対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-ac3
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Enhanced Audio 仕様のうち FourCC `ac-3` に対応し、AC-3 (Dolby Digital) を含む音声フレームを送受信できるようにする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義されているコーデックであり、完全準拠の観点から実装は必要である。
AC-3 は放送・配信領域で広く使われるが、時雨堂製品での直接的な需要は現時点で限定的なため、優先度は Medium とする。

## 現状

- ExAudioTagHeader 基盤 (SoundFormat::ExHeader 分岐、`AudioPacketType`、`AudioFourCc`、`MultichannelConfig`、`AudioFrame` のデータモデル再構成) は issue 0009 で導入される。本 issue はその基盤の上に `ac-3` のペイロード処理を追加するスコープであり、0009 の完了が前提となる
- 0009 完了時点では、`ac-3` を受信すると基盤の分岐で `ErrorKind::Unsupported` が返る。本 issue はこの分岐を AC-3 実装で置き換える
- AC-3 は仕様上 SequenceStart のペイロードが定義されていない (仕様 988-1022 行の SequenceStart セクションに AC-3 エントリなし)。CodedFrames のみに ATSC A/52 ビットストリームを持つ

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`refs/enhanced-rtmp-v2.md` の 735 行以降) に準拠する
- 0009 で整備される ExAudioTagHeader 基盤の上に、FourCC が `ac-3` (仕様 854 行) の場合の分岐とペイロード処理を追加する
- 各 AudioPacketType の AC-3 ペイロード (仕様の該当行):
  - `CodedFrames`: ボディは ATSC A/52 ビットストリーム (仕様 1025-1029 行。`Ac3CodedData`)。E-AC-3 (`ec-3`) と同じ CodedFrames ブロックを共有する (仕様 1025 行: `audioFourCc == AudioFourCc.Ac3 || audioFourCc == AudioFourCc.Eac3`)。0013 (E-AC-3) 実装時にこの分岐を共通化する。パース結果は raw バイトを `AudioFrame::data` にそのまま格納する。フレーム境界の解釈は上位レイヤーの責務とする。エンコード時も `data` をそのまま書き出す
  - `SequenceStart`: AC-3 には仕様上定義されていない (仕様 988-1022 行の SequenceStart セクションに AC-3 エントリなし)。デコード時に `ac-3` + SequenceStart を受信した場合は `Error::invalid_data` とする。エンコード時に `ac-3` + SequenceStart の `AudioFrame` を渡された場合も `Error::invalid_data` とする
  - `SequenceEnd`: ボディ無し (仕様 984-986 行。SequenceEnd はコーデック非依存)。デコード時に残余バイトがあれば `Error::invalid_data`、エンコード時に `data` が非空ならエラーとする (0009 の mp4a 分岐と同じ方針)
  - 上記以外の AudioPacketType (MultichannelConfig / Multitrack / ModEx) の扱いは 0009 のエラー分類に従う (MultichannelConfig は 0009 で実装済み、Multitrack は 0014、ModEx は 0015)。MultichannelConfig は仕様上 FourCC 分岐より前に処理される (仕様 954 行) ため、コーデック非依存で動作する。0009 が `ac-3` を `ErrorKind::Unsupported` とするのは SequenceStart / CodedFrames のペイロード分岐であり、MultichannelConfig / SequenceEnd には影響しない
- AC-3 は ATSC A/52 ビットストリーム内にチャンネル構成情報を含む自己記述型コーデックである。0009 の MultichannelConfig が ac-3 でも動作する (仕様 954 行で FourCC 分岐より前に処理されるコーデック非依存パスであり、0009 のテストで確認済み。0012 での追加実装は不要)
- AC-3 固有の新規型は不要 (SequenceStart が定義されていないため)。`src/lib.rs` への re-export 追加も不要。`AudioFormat::ExAc3` バリアントは 0009 が全 FourCC 分を事前定義するため (0009 設計方針の `AudioFourCc` 全列挙値定義)、0012 での追加は不要
- エラー分類: 0009 の方針を継承する。AC-3 固有の追加分類は上記の SequenceStart 拒否 (`Error::invalid_data`) のみ
- 相互接続に関する制約 (本 issue のスコープ外だが依存として明記):
  - FourCC 対応の connect コマンドでの signaling は仕様上 MUST (仕様 848-850 行) であり、0017 で対応する。実際の相互接続には 0017 の完了が前提となる

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `ac-3` の AudioTagHeader をデコード/エンコードできる (CodedFrames / SequenceEnd)
- `ac-3` + SequenceStart のデコード/エンコードが `Error::invalid_data` になる
- CodedFrames のデコード/エンコードが raw バイトの round-trip で正しく動作する (フレーム分割は行わず、`AudioFrame::data` にそのまま格納)
- ユニットテストで上記の各ケースが確認できる
- pbt: AC-3 を含む `AudioFrame` の Generator が追加され、round-trip プロパティが通る
- fuzz: AudioFrame デコードの fuzz は既存 `fuzz_flv.rs` がカバーするため新設せず、AC-3 のコーパス種を `fuzz/corpus/` に追加する
- 既存の legacy / 0009 で導入された他コーデックの動作に回帰が無い (既存テストがすべて通る)
- CHANGES.md に ADD として記載されている

## 解決方法

1. `src/flv.rs`
   - 0009 の FourCC 分岐で `ac-3` を AC-3 経路 (設計方針の各 AudioPacketType 処理) につなぐ
   - `ac-3` + SequenceStart を `Error::invalid_data` で拒否する分岐を追加する
2. `tests/` / `pbt/` / `fuzz/`
   - 完了条件に列挙したテスト・Generator・コーパスを追加する
3. `CHANGES.md`
   - ADD として記載する (`shiguredo-changelog` スキル参照)
