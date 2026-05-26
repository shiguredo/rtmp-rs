# fuzz_rtmp_command を削除する

- Priority: Medium
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/fix-fuzz-rtmp-command

## 目的

現状の `fuzz_rtmp_command` は実質的に機能しておらず、fuzzing のリソースを浪費している。削除して、コマンドパースの fuzzing は `fuzz_rtmp_message` に任せる。

## 優先度根拠

Medium。機能していない fuzz ターゲットが存在すること自体が、fuzzing カバレッジに対する誤った安心感を与える。ただし、`fuzz_rtmp_message` が完全なコマンドパースパスをカバーしているため、セキュリティ上の緊急性はない。

## 現状

`fuzz/fuzz_targets/fuzz_rtmp_command.rs` には以下の問題がある:

1. `RtmpCommand::from_message()` に渡す `args` が常に空の `vec![]` である。以下のコマンドは `args.first().ok_or_else(...)` で即座にエラーになり、パースロジックに到達しない:
   - `publish`（`rtmp_command.rs:317-320`）
   - `play`（`rtmp_command.rs:380-383`）
   - `deleteStream`（`rtmp_command.rs:436-439`）
   - `getStreamLength`（`rtmp_command.rs:459-462`）
   - `_result`（`rtmp_command.rs:493-496`）
   - `onStatus`（`rtmp_command.rs:539-541`）

2. AMF0 でしかデコードしておらず、AMF3 のパスが一切テストされない

3. `fuzz_rtmp_message` が `RtmpMessageDecoder` 経由でチャンクデコード -> メッセージデコード -> AMF デコード -> コマンドパースの全パスを通るため、`fuzz_rtmp_command` の役割は完全に代替されている

## 設計方針

- `fuzz/fuzz_targets/fuzz_rtmp_command.rs` を削除する
- `fuzz/Cargo.toml` から対応する `[[bin]]` エントリを削除する

## 完了条件

- `fuzz_rtmp_command.rs` が削除されている
- `fuzz/Cargo.toml` から `fuzz_rtmp_command` の `[[bin]]` エントリが削除されている
- `cargo fuzz list` に `fuzz_rtmp_command` が表示されない
- `cargo fuzz build` が通る
