# FLV 音声タグの SoundSize（8 bit / 16 bit）の解釈を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-flv-sound-size
- Polished: {YYYY-MM-DD}

## 目的

FLV 音声タグの SoundSize ビットの解釈を仕様に合わせ、8 bit と 16 bit のサンプルサイズを正しく通知・解釈できるようにする。

## 現状

- FLV 仕様（`refs/enhanced-rtmp-v2.md` の Pre 2023 AudioTagHeader）では、SoundSize は 0 = 8-bit samples、1 = 16-bit samples と定義されている。
- `src/flv.rs` の `decode_audio_frame` は `let is_8bit_sample = (header & 0x02) != 0;` としており、ビット 1 が立っていると 8 bit と解釈する。仕様とは逆である。
- `src/flv.rs` の `encode_audio_frame` は `(frame.is_8bit_sample as u8) << 1` でビットを立てるため、`is_8bit_sample = false`（16 bit）のフレームが SoundSize 0（8 bit 相当）で送信される。
- エンコードとデコードが同じ誤解釈を共有しているため、ライブラリ内の往復テストでは検出できない。

### 再現条件

1. `decode_audio_frame(&[0x2F], RtmpTimestamp::ZERO)` を呼ぶ。`0x2F` は SoundFormat 2（MP3）、SoundRate 3（44 kHz）、SoundSize 1（16 bit）、SoundType 1（stereo）。
2. `is_8bit_sample` を確認する。

期待は `false`（16 bit）。実際は `true`（8 bit）になる。

逆方向では、`is_8bit_sample = false` の `AudioFrame` を `encode_audio_frame` でエンコードすると、SoundSize ビットが 0 になる。

## 設計方針

- `encode_audio_frame` と `decode_audio_frame` の両方で、SoundSize ビットの意味を仕様に合わせる。`is_8bit_sample` が `true` のときビット 1 を 0、`false` のときビット 1 を 1 とする。
- 公開 API の `AudioFrame::is_8bit_sample` の名前と意味は維持する。挙動が変わるため、CHANGES.md に CHANGE として記載する。
- 0x2F（16 bit）と 0x2E（8 bit）などのゴールデンバイトテストを追加し、往復テストだけに頼らないようにする。
- AAC など SoundSize が無視されるフォーマットの扱いは現行どおりとする。

## 完了条件

- `0x2F` のデコードで `is_8bit_sample = false` になる。
- 16 bit の `AudioFrame` のエンコードで SoundSize ビットが 1 になる。
- 8 bit / 16 bit の両方でエンコードとデコードの往復が一致する。
- 既存の音声フォーマットと AAC のテストに回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `src/flv.rs` の `decode_audio_frame` の SoundSize の解釈を反転する。
2. `src/flv.rs` の `encode_audio_frame` の SoundSize の書き込みを反転する。
3. ゴールデンバイトテストと往復テストを追加する。
4. `CHANGES.md` に CHANGE として記載する。
