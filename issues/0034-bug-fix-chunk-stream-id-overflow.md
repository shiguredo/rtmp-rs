# メッセージストリーム ID からチャンクストリーム ID の変換で発生するオーバーフローを修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-chunk-stream-id-overflow
- Polished: {YYYY-MM-DD}

## 目的

メッセージストリーム ID からチャンクストリーム ID を求める処理で、`u32` の加算オーバーフローによるパニックや不正な ID の生成を防ぐ。

## 現状

- `src/rtmp_chunk.rs` の `RtmpChunkStreamId::from_message_stream_id` は `(message_stream_id.get() + 2).min(65599)` を計算する。
- `u32::MAX - 1` と `u32::MAX` を渡すと加算がオーバーフローする。
  - デバッグビルドでは `attempt to add with overflow` でパニックする。
  - リリースビルドでは 2^32 で折り返し、それぞれ 0 と 1 になる。
- 0 と 1 は `RtmpChunkStreamId` の予約値であり、`RtmpChunkStreamId::new` は受理しない。基本ヘッダー上では 0 は 2 バイト形式、1 は 3 バイト形式の先頭バイトとして解釈されるため、チャンク列が壊れる。
- `src/rtmp_connection.rs` の `RtmpMessageChannel::feed_send_message` は、メッセージのメッセージストリーム ID からチャンクストリーム ID をこの関数で求めるため、公開 API から到達できる。

### 再現条件

1. `RtmpChunkStreamId::from_message_stream_id(RtmpMessageStreamId::new(u32::MAX - 1))` を呼ぶ。

デバッグビルドではパニック、リリースビルドでは 0 が返る。期待は 65599 への飽和。

## 設計方針

- `message_stream_id.get().saturating_add(2).min(65599)` のように、加算を飽和させる。
- 0 と 1 が返らないことを不変条件とする。
- メッセージストリーム ID の上限付近のテストを追加する。
- チャンクストリーム ID の対応付けがこのライブラリ固有の規則であることは既存の doc に書かれているため、その説明に「加算は飽和する」ことを追記する。

## 完了条件

- `u32::MAX - 3`、`u32::MAX - 2`、`u32::MAX - 1`、`u32::MAX` のすべてでパニックせず、有効範囲（2〜65599）のチャンクストリーム ID を返す。
- 既存のメッセージストリーム ID（0〜65597）の対応付けに変更がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `RtmpChunkStreamId::from_message_stream_id` の加算を `saturating_add` に変更する。
2. 上限付近のテストを追加する。
3. doc コメントを更新する。
