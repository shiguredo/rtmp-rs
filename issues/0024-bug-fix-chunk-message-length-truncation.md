# チャンクのメッセージ長が 24 ビットで切り詰められる問題を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-chunk-message-length-truncation
- Polished: {YYYY-MM-DD}

## 目的

RTMP のメッセージ長フィールドが 24 ビットであることをエンコーダで検証し、16 MiB を超えるメッセージが無言で破損したストリームとして送信されないようにする。

## 現状

- RTMP 仕様 5.3.1.2 節のメッセージヘッダーでは、message length は 3 バイトである。
- `src/rtmp_chunk_encoder.rs` の `RtmpChunkEncoder::encode_message_header` は、message length を `write_u24(chunk_stream.message_size as u32)` で書き込む。`write_u24` は下位 3 バイトだけを書き込むため、
  0xFFFFFF を超える長さが無言で切り詰められる。
- `RtmpChunkEncoder::encode` は戻り値が `()` であり、長さの検証エラーを返す経路がない。
- メッセージ長が切り詰められると、受信側は小さいメッセージとして完了したと解釈し、残りのバイトを次のチャンクのヘッダーとして解析するため、ストリーム全体が壊れる。

### 再現条件

1. `RtmpChunk` を `message_type = RtmpMessageType::DataAmf0`、`payload` を 16 MiB + 1 バイト（0x01000001 バイト）の 0xA5 で構築する。
2. `RtmpChunkEncoder::encode` でエンコードする。
3. エンコード結果の message length フィールドを読み、`RtmpChunkDecoder::decode` でデコードする。

期待は 16 MiB + 1 バイトのメッセージとして扱われるか、エンコード時にエラーになること。実際は message length が 1 になり、デコーダは 1 バイトのメッセージとして完了し、残りがチャンクヘッダーとして解釈される。

## 設計方針

- `RtmpChunkEncoder::encode` を `Result<(), Error>` に変更し、`message_size > 0xFFFFFF` を `ErrorKind::InvalidData`（または `ErrorKind::InvalidInput`）として拒否する。
- もしくはメッセージ層で 0xFFFFFF を超えるペイロードを複数メッセージに分割する。RTMP のメッセージ分割は Aggregate など別の仕組みが必要なため、本 issue では拒否する方針を第一候補とする。
- 境界として 0xFFFFFF は成功、0x1000000 はエラーになることをテストで固定する。
- 公開 API の破壊的変更になるため、CHANGES.md に CHANGE として記載する。

## 完了条件

- 0xFFFFFF バイトのペイロードは従来どおりエンコードできる。
- 0x1000000 バイト以上のペイロードはエンコード時に `Error` を返す。
- エラー時に送信バッファへ部分的なチャンクが残らない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `RtmpChunkEncoder::encode` の戻り値を `Result` に変更する。
2. `encode_message_header` の前に `message_size` を検証する。
3. 呼び出し側（`RtmpMessageEncoder::encode`、`RtmpMessageChannel::feed_send_message`、接続 API）の戻り値の扱いを修正する。
4. 境界値のテストを追加し、`CHANGES.md` に CHANGE として記載する。
