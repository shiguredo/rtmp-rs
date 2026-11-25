# play の開始位置を省略した再生要求を受理する

- Created: 2026-11-25
- Completed: 2026-11-25
- Branch: feature/fix-play-start-default
- Polished: 2026-11-25

## 目的

クライアントが `play` コマンドの開始位置を省略した場合も、仕様の既定値で再生要求を受理する。

## 現状

`rtmp_command` の `RtmpPlayCommand::from_message` は、引数の 2 番目を開始位置として必須にしている。開始位置がない場合は `InvalidData` を返すため、`RtmpServerConnection` は再生要求イベントを通知しない。

[Adobe RTMP 仕様（2012 年 12 月版）§7.2.2.1](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=39) は、開始位置を省略可能な数値とし、既定値を `-2` と明記している。`-2` はライブ配信を優先し、なければ録画を探し、どちらもなければ新しいライブ配信を待つ。

### 再現条件

1. サーバーのハンドシェイクを完了し、`connect` と `createStream` を処理する。
2. `play` コマンドの引数をストリーム名だけにして送る。
3. `feed_recv_buf` は `InvalidData` を返し、状態は `MediaStreamCreated` のままになる。

## 設計方針

- 開始位置の引数がない場合だけ `-2` を使う。明示された数値はそのまま保持する。
- 明示された値が数値でない場合の `InvalidData` は変更しない。
- `prop_rtmp_command` の既存の性質テストは、開始位置の省略を拒否する前提になっている。省略時に `-2` を使うことを検証するテストへ置き換える。サーバー側の公開 API の性質テストにも、開始位置を省略した再生要求を加える。
- 録画再生やプレイリストの対応、再生方針の変更は行わない。送信する `play` コマンドの形式も変更しない。
- 開始位置の既定値の仕様が変わった場合は、この扱いを見直す。

## 完了条件

- 開始位置を省略した `play` コマンドで、`RtmpPlayCommand` の `start` が `-2` になり、ストリーム名とトランザクション ID は引数の値を保つ。
- サーバーの公開 API で、開始位置を省略した `play` を送ると `PlayPending` へ遷移し、`PlayRequested` が通知される。
- 開始位置を明示した既存の往復と、数値でない値を拒否する挙動に回帰がない。省略を拒否する既存の性質テストは、省略時の既定値を確認する内容に置き換わっている。
- 上記を公開 API の性質テストで検証する。

## 解決方法

1. `RtmpPlayCommand::from_message` で、開始位置の引数がある場合は数値として読み取り、ない場合だけ仕様の既定値 `-2.0` を使うようにした。明示された値が数値でない場合の `InvalidData` と、ストリーム名が必須であることは変更していない。
2. `RtmpPlayCommand::start` と `RtmpCommand::from_message` の公開説明を、省略時の既定値と省略可能な引数の扱いに合わせた。送信する `play` コマンドの形式と再生方針は変更していない。
3. `prop_rtmp_command` の開始位置の省略を拒否していた性質テストを、省略時の既定値と名前・トランザクション ID の保持を確認する内容に置き換えた。`prop_rtmp_server_connection` に、開始位置を省略した `play` で `PlayPending` へ遷移し `PlayRequested` が通知されることを確認する性質テストを追加した。
