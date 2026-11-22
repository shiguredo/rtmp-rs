# FLV の SoundFormat 0（Linear PCM, platform-endian）を受理する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-flv-linear-pcm
- Polished: {YYYY-MM-DD}

## 目的

FLV 仕様で定義されている SoundFormat 0（Linear PCM, platform-endian）をデコード・エンコードできるようにする。

## 現状

- `refs/enhanced-rtmp-v2.md` の Pre 2023 AudioTagHeader の表では、SoundFormat 0 = Linear PCM, platform-endian と定義されている。
- `src/flv.rs` の `decode_audio_frame` の SoundFormat 対応表に 0 がなく、`0x0E`（SoundFormat 0、SoundRate 3、SoundSize 1、SoundType 0）を `ErrorKind::InvalidData` で拒否する。
- `src/media.rs` の `AudioFormat` enum にも 0 に対応する variant がない。
- `encode_audio_frame` も Linear PCM を出力できない。

### 再現条件

1. `decode_audio_frame(&[0x0E], RtmpTimestamp::ZERO)` を呼ぶ。

期待は SoundFormat 0（Linear PCM, platform-endian）の `AudioFrame`。実際は `Invalid audio format: 0` のエラーになる。

## 設計方針

- `AudioFormat` に Linear PCM（platform-endian）を表す variant を追加する。
- `decode_audio_frame` と `encode_audio_frame` の対応表に 0 を追加する。
- 既存 variant の数値は変更しない。`AudioFormat` は `#[non_exhaustive]` を使っていないため、variant 追加は破壊的変更になる。CHANGES.md に CHANGE として記載する。
- Nellymoser などの既存フォーマットの対応に回帰がないことを確認する。

## 完了条件

- `0x0E` を含む SoundFormat 0 の音声タグをデコードできる。
- `AudioFormat::LinearPcmPlatformEndian` 相当のフレームをエンコードすると SoundFormat 0 のバイト列になる。
- デコードとエンコードの往復が一致する。
- 既存 SoundFormat のテストに回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `src/media.rs` の `AudioFormat` に SoundFormat 0 の variant を追加する。
2. `src/flv.rs` の `decode_audio_frame` / `encode_audio_frame` の対応表を更新する。
3. 0x0E のゴールデンバイトテストと往復テストを追加する。
4. `CHANGES.md` に CHANGE として記載する。
