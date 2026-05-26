# RtmpUrl::parse() の fuzz ターゲットを追加する

- Priority: High
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/add-fuzz-rtmp-url

## 目的

`RtmpUrl::parse()` および `RtmpUrl::parse_with_stream_name()` は外部文字列入力をパースする関数であり、任意入力に対するパニック安全性を保証するために fuzzing が必要である。現状、この関数に対する fuzz ターゲットが存在しない。

## 優先度根拠

URL パースは外部入力の最初の接点であり、パニックが発生するとプロセス全体がクラッシュする。IPv6 ブラケット処理やポート分割など境界条件の多い文字列パースロジックが含まれており、fuzzing によるクラッシュ検出の効果が高い。

## 現状

`src/rtmp_url.rs` の `RtmpUrl::parse()` と `RtmpUrl::parse_with_stream_name()` に対する fuzz ターゲットが存在しない。PBT も存在しないが、PBT の追加は本 issue のスコープ外とする。

## 設計方針

- `fuzz/fuzz_targets/fuzz_rtmp_url.rs` を新規作成する
- `shiguredo_rtmp::RtmpUrl` を直接インポートする（`fuzz_client_connection.rs` と同じパターン）
- 1 つの `fuzz_target!` コールバック内で `parse()` と `parse_with_stream_name()` の両方を実行する
- 入力バイト列全体を `core::str::from_utf8()` で UTF-8 文字列に変換し、失敗した場合は早期リターンする
- 変換した文字列に対して `RtmpUrl::parse()` を呼び出し、成功した場合は `to_string()` も呼び出す
- `parse_with_stream_name()` の fuzz は入力バイト列を 2 分割して行う:
  - 先頭 1 バイトを分割位置のインデックスとして使う
  - 残りのバイト列に対して `split_pos % remaining.len()` で分割位置を正規化する
  - 入力が 2 バイト未満の場合は `parse_with_stream_name()` の fuzz をスキップする
  - 前半（URL）・後半（`stream_name`）それぞれに `core::str::from_utf8()` を適用し、どちらかが失敗した場合はスキップする
  - 成功した場合は `to_string()` も呼び出す
- `FromStr` trait の実装は現時点で `parse()` への単純な委譲のみであるため、個別の fuzz は不要
- `fuzz/Cargo.toml` に以下の `[[bin]]` エントリを追加する:

```toml
[[bin]]
name = "fuzz_rtmp_url"
path = "fuzz_targets/fuzz_rtmp_url.rs"
test = false
doc = false
bench = false
```

## 完了条件

- `fuzz_rtmp_url` ターゲットが追加されている
- `cargo fuzz list` に `fuzz_rtmp_url` が表示される
- `cargo fuzz run fuzz_rtmp_url -- -runs=0` でビルドが通る
- 60 秒間の fuzzing 実行でクラッシュが発生しない
