# FLV の SoundFormat 0（Linear PCM, platform-endian）を受理する

- Created: 2026-11-22
- Completed: 2026-11-25
- Branch: feature/change-flv-linear-pcm
- Polished: 2026-11-22

## 目的

既存データとの互換性のため、legacy FLV の SoundFormat 0（Linear PCM, platform-endian）のヘッダーを識別し、音声データを変更せずに保持・再出力できるようにする。

## 現状

- [Adobe FLV 仕様 10.1 §E.4.2.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=76) と Enhanced RTMP v2 の「Pre 2023 AudioTagHeader」の表は、SoundFormat 0 をプラットフォームのバイト順のリニア PCM、3 をリトルエンディアンのリニア PCM と定義している。
- `src/flv.rs` の `decode_audio_frame` の SoundFormat 対応表に 0 がなく、`0x0E`（SoundFormat 0、SoundRate 3、SoundSize 1、SoundType 0）を `ErrorKind::InvalidData` で拒否する。
- `src/media.rs` の `AudioFormat` に 0 に対応するバリアントがないため、`encode_audio_frame` は形式 0 を出力できない。形式 3 はすでに `LinearPcmLittleEndian = 3` で読み書きできる。

### 再現条件

`decode_audio_frame(&[0x0E, 0x12, 0x34], RtmpTimestamp::ZERO)` を呼ぶ。期待値は SoundFormat 0、モノラル、16 ビットの `AudioFrame`。現行実装は形式の判定で `ErrorKind::InvalidData` を返し、その理由は `Invalid audio format: 0` になる。

## 設計方針

- `AudioFormat::LinearPcmPlatformEndian = 0` を追加する。`decode_audio_frame` の SoundFormat 判定で 0 を新バリアントに対応付ける。`encode_audio_frame` は既存の列挙値から上位 4 ビットを作る処理で 0 を出力する。既存バリアントの判別値と、形式 0 以外の SoundFormat の対応付けは変更しない。
- 形式 0 の 16 ビットサンプルのバイト順は、[Adobe FLV 仕様 10.1 §E.4.2.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=77) に従い、ファイルを作成したプラットフォームのバイト順に依存する。実行ホストのバイト順を使った変換や推定、形式 3 への正規化、サンプルの妥当性検査は行わず、`AudioFrame::data` をそのまま扱う。形式 0 は明示的に指定された場合だけ出力し、新規データの生成には Adobe 仕様の非推奨を踏まえ形式 3 を案内する。
- SoundSize の共通規則を修正する 0023 を先に実装する。本 issue では SoundSize の条件分岐を追加しない。実装順は 0023 → 本 issue → Enhanced Audio の 0009 とする。本 issue は現行の公開型を前提とする。0009 の形式 0 を除外・拒否する記述は本修正後には適用しない。0009 に着手する前にその設計と非回帰の基準を同期し、形式 0 の受理を維持する。
- `AudioFormat` は `#[non_exhaustive]` ではないため、利用側の網羅的な `match` には新しい分岐が必要になる。

## 完了条件

- 上記の入力から `format = AudioFormat::LinearPcmPlatformEndian`、`sample_rate = AudioSampleRate::Khz44`、`is_8bit_sample = false`、`is_stereo = false`、`is_aac_sequence_header = false`、`timestamp = RtmpTimestamp::ZERO`、`data = [0x12, 0x34]` を得る。このフィールドを指定したフレームから、追加ヘッダーを入れず `[0x0E, 0x12, 0x34]` が得られる。SoundSize が 0 の `[0x0C, 0x12]` では `is_8bit_sample = true`、`data = [0x12]` になる。エンコードとデコードをそれぞれ固定値に照合する。
- 既存の FLV と RTMP メッセージの往復 PBT に形式 0 を含め、ヘッダーの値と `data` が往復することを確認する。非 AAC として `is_aac_sequence_header = false` のフレームを使う。形式 3 や Nellymoser など既存形式の振る舞いは 0023 適用後のものを維持する。単体テストを追加するのは PBT で実現できない確認に限る。

## 解決方法

- `AudioFormat::LinearPcmPlatformEndian = 0` を追加した。エンコーダーは既存のヘッダー処理で形式 0 を書き、デコーダーは形式 0 を同バリアントとして読み取る。音声データを変換せず、形式 3 や他の音声形式は変更しない。
- 公開文書に、16 ビットのバイト順は作成元に依存し、新規データには形式 3 を使うことを記載した。既存の 16 ビット PCM を形式 3 にする場合は、形式番号だけでなくサンプルをリトルエンディアンにする必要がある。
- 8 ビットと 16 ビットの固定バイト列をエンコード・デコードそれぞれで照合した。FLV と RTMP の往復性質テストに形式 0 を追加し、既存の形式が変わらないことも確認した。
- `AudioFormat` を網羅する `match` には新しい分岐が必要になる。
