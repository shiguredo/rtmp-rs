# AvcSequenceHeader::from_bytes() の fuzz ターゲットを追加する

- Priority: Medium
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/add-fuzz-avc-sequence-header

## 目的

`AvcSequenceHeader::from_bytes()` はバイナリパーサーであり、ネストされた長さプレフィクス付きフィールド（SPS/PPS リスト）を手動でオフセット管理しながらパースしている。任意入力に対するパニック安全性を保証するために fuzzing が必要である。

## 優先度根拠

Medium。`fuzz_flv` は `decode_audio_frame` / `decode_video_frame` をカバーするが、`AvcSequenceHeader::from_bytes()` には到達しない。FLV デコーダーはビデオペイロードの中身（AVC シーケンスヘッダーの内部構造）まで踏み込まず、ペイロードをそのまま `data` フィールドに格納するためである。`AvcSequenceHeader::from_bytes()` は利用者が明示的に呼び出す公開 API であり、不正な入力に対する耐性を検証する必要がある。

## 現状

`src/media.rs` の `AvcSequenceHeader::from_bytes()` はオフセットを手動管理するパーサーで、以下の処理を行う:

- configuration version の検証
- SPS 数の読み取り（5 ビットマスク）とサイズ上限チェック
- SPS ごとの長さプレフィクス（2 バイト）とデータの読み取り
- PPS 数の読み取り（8 ビット）とサイズ上限チェック
- PPS ごとの長さプレフィクス（2 バイト）とデータの読み取り

単体テストは存在するが、fuzz ターゲットは存在しない。

## 設計方針

- `fuzz/fuzz_targets/fuzz_avc_sequence_header.rs` を新規作成する
- 入力バイト列を `AvcSequenceHeader::from_bytes()` に渡し、成功した場合は `to_bytes()` も呼び出してラウンドトリップを検証する
- `fuzz/Cargo.toml` に `[[bin]]` エントリを追加する

## 完了条件

- `fuzz_avc_sequence_header` ターゲットが追加されている
- `cargo fuzz list` に `fuzz_avc_sequence_header` が表示される
- `cargo fuzz run fuzz_avc_sequence_header -- -runs=0` でビルドが通る
