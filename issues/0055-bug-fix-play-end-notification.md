# 再生終了の通知で再生状態を終了する

- Created: 2026-11-25
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-play-end-notification
- Polished: {YYYY-MM-DD}

## 目的

サーバーから再生終了が通知されたとき、再生クライアントが `Playing` のまま残らず、終了を利用側へ通知する。

## 現状

`rtmp_client_connection` の `RtmpPlayClientConnection` は、`RtmpUserControlEvent::StreamEof` を `RtmpConnectionEvent::user_control_event_ignored` として通知し、`onStatus` の `NetStream.Play.Stop` も `command_ignored` として通知する。どちらの場合も状態は `Playing` のままで、再生が終わったことを示す状態遷移や専用のイベントはない。

[Adobe RTMP 仕様（2012 年 12 月版）§7.1.7](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=27) は、`Stream EOF` を「そのストリームのデータ再生が要求どおり終了したこと」をクライアントへ通知するイベントとし、追加のコマンドがなければそれ以上データを送らず、クライアントはそのストリーム向けに受信したメッセージを破棄すると定義している。

### 再現条件

1. 再生クライアントを `Playing` まで遷移させる。
2. `Stream EOF`（イベント型 1、対象ストリーム ID）のユーザーコントロールメッセージを送る。または `onStatus` で `level` が `status`、`code` が `NetStream.Play.Stop` のメッセージを送る。
3. `next_event` と `state` を確認する。

現行は無視イベントが 1 件増えるだけで、状態は `Playing` のままになる。期待は、再生終了が利用側へ通知され、以後そのストリーム向けのメディアを受信しても再生中として扱わないこと。

## 設計方針

- 再生クライアントで、対象ストリーム ID が一致する `Stream EOF` を受けたら再生終了として扱う。`NetStream.Play.Stop` も同じ扱いにする。
- 再生終了を表すイベントを追加し、状態を `Playing` から `Connected` に戻す。TCP 接続は切断しない。`Disconnecting` にしない。
- `NetStream.UnpublishNotify` は配信側の停止通知なので、再生終了とは区別する。
- 再生していない状態で受けた場合や、ストリーム ID が一致しない場合は、`Playing` を維持して無視イベントとして扱う。
- 録画再生の終端位置やプレイリストの遷移は対象外とし、状態の終了だけを扱う。

## 完了条件

- 公開 API で、`Playing` のときに `Stream EOF` または `NetStream.Play.Stop` を送ると、再生終了のイベントが通知され、状態が `Connected` になる。
- 終了後に同じストリーム向けのメディアイベントを受けても、再生終了の通知が重複しない。
- ストリーム ID が一致しない `Stream EOF` と、再生していない状態で受けた終了通知では状態が変わらない。
- 上記を公開 API の性質テストで検証する。
