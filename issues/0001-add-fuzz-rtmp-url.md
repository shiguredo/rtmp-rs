# RtmpUrl::parse() の fuzz ターゲットを追加する

- Priority: High
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/add-fuzz-rtmp-url

## 目的

`RtmpUrl::parse()` および `RtmpUrl::parse_with_stream_name()` は外部文字列入力をパースする関数であり、任意入力に対するパニック安全性を保証するために fuzzing が必要である。現状、この関数に対する fuzz ターゲットが存在しない。

## 優先度根拠

High。URL パースは外部入力の最初の接点であり、パニックが発生するとプロセス全体がクラッシュする。IPv6 アドレスのブラケット処理、`rfind(':')` によるポート分割、`rsplit_once('/')` による app/stream_name 分割など、境界条件の多い文字列パースロジックが含まれており、fuzzing によるクラッシュ検出の効果が高い。

## 現状

`src/rtmp_url.rs` には単体テストが存在するが、fuzz ターゲットが存在しない。以下のパースロジックが fuzzing の対象になる:

- scheme の判定（`rtmp` / `rtmps`）
- `://` の分割
- IPv6 アドレス（`[...]` 形式）のパース
- `:` によるホストとポートの分割
- `/` による app と stream_name の分割
- ポート番号の `u16` パース

## 設計方針

- `fuzz/fuzz_targets/fuzz_rtmp_url.rs` を新規作成する
- 入力バイト列を UTF-8 文字列に変換し、`RtmpUrl::parse()` と `RtmpUrl::parse_with_stream_name()` の両方を呼び出す
- `fuzz/Cargo.toml` に `[[bin]]` エントリを追加する

## 完了条件

- `fuzz_rtmp_url` ターゲットが追加されている
- `cargo fuzz list` に `fuzz_rtmp_url` が表示される
- `cargo fuzz run fuzz_rtmp_url -- -runs=0` でビルドが通る
