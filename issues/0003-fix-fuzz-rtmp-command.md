# fuzz_rtmp_command の args 生成を修正する

- Priority: Medium
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/fix-fuzz-rtmp-command

## 目的

現状の `fuzz_rtmp_command` は `RtmpCommand::from_message()` に渡す `args` が常に空の `vec![]` であり、`args` を参照するコマンド（`publish`、`play`、`deleteStream`、`getStreamLength`、`_result`、`onStatus`）のパースロジックに到達しない。`args` にも fuzzer 生成の AMF 値を渡すように修正し、これらのコマンドのパース入口（`args.first()` の行）を超えられるようにする。

## 優先度根拠

`fuzz_rtmp_command` は `RtmpCommand::from_message()` を直接テストする唯一の fuzz ターゲットである。`fuzz_server_connection` と `fuzz_client_connection` もコマンドパースに到達するが、ハンドシェイクを含むステートマシンを経由するため到達率が低い。既存リストの 7 コマンド中 5 コマンドが args 不足でパースロジックに到達せず、さらに `getStreamLength` がリストに未登録であるため、計 6 コマンドのパースロジックが fuzzing されていない。

## 現状

`fuzz/fuzz_targets/fuzz_rtmp_command.rs` は以下の問題を抱えている:

1. `RtmpCommand::from_message()` に渡す `args` が常に空の `vec![]` であるため、`args.first().ok_or_else(...)` で即座にエラーになり、`publish`、`play`、`deleteStream`、`_result`、`onStatus` の 5 コマンドのパースロジックに到達しない。`connect` と `createStream` は `args` を使わないため機能している
2. コマンド名リストに `getStreamLength` が含まれていないため、`_` アームで `Ignore` になりパースロジックが呼ばれない
3. AMF0 でしかデコードしておらず、AMF3 でデコードした AmfValue を入力とするパスがテストされない。ただし `RtmpCommand::from_message()` 自体は `AmfVersion` を引数に取らず、AMF バージョンに依存しないため、AMF3 対応は本 issue のスコープ外とする

## 設計方針

- `fuzz_rtmp_command.rs` を修正し、入力バイト列から `args` 用の AMF 値もデコードする
- 入力バイト列全体を AMF0 値として連続デコードし、最初の値を `object`、2 番目以降を `args` として `RtmpCommand::from_message()` に渡す
- デコードできる AMF 値が 1 つもない場合は早期リターンする
- コマンド名リストに `getStreamLength` を追加する（既存リストには含まれていないが、`RtmpCommand::from_message()` で明示的にハンドリングされるコマンドである）
- `transaction_id` は既存の `TransactionId::from_f64(1.0)` のままとする。`transaction_id` の fuzzing は本 issue のスコープ外とする

## 完了条件

- `fuzz_rtmp_command.rs` が修正されている
- 入力バイト列から AMF 値が 2 つ以上デコードされた場合に `args` が非空で `RtmpCommand::from_message()` に渡されるコードパスが存在する
- コマンド名リストに `getStreamLength` が追加されている
- `cargo fuzz run fuzz_rtmp_command -- -runs=0` でビルドが通る
- 60 秒間の fuzzing 実行でクラッシュが発生しない
