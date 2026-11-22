# 標準メッセージ型 16 / 19 / 22 を未知型として切断しないようにする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-message-type-coverage
- Polished: {YYYY-MM-DD}

## 目的

RTMP 仕様で定義されているメッセージ型 16 / 19 / 22 を受信したときに、接続をエラーで切断しないようにする。

## 現状

- `src/rtmp_message.rs` の `RtmpMessageType::from_type_id` は 1、2、3、4、5、6、8、9、15、17、18、20 のみを受理し、それ以外を `unknown message type` の `ErrorKind::InvalidData` にする。
- RTMP 仕様 6 章では、Shared Object メッセージに 16（AMF3）と 19（AMF0）、Aggregate メッセージに 22 が定義されている。
- チャンクデコーダは `decode_message_header` で `RtmpMessageType::from_type_id` を呼ぶため、これらの型を受信するとメッセージヘッダーの解析時点でエラーになる。

### 再現条件

1. fmt0 のチャンクでメッセージ型 22（Aggregate）、メッセージ長 0、メッセージストリーム ID 0 を渡す。
2. `RtmpMessageDecoder::decode` を呼ぶ。

期待はメッセージを無視して接続を継続すること。実際は `unknown message type: 22` のエラーになる。型 16 / 19 も同様。

## 設計方針

- `RtmpMessageType` に 16 / 19 / 22 を追加するか、未知型を許容する表現に変更する。
- 本ライブラリが処理しないメッセージ型は、接続を切断せずに `RtmpConnectionEvent::MessageIgnored` などで通知する。既存の `MessageIgnored` イベントの使い方と整合させる。
- Aggregate メッセージの内部チャンクを展開するかどうかは別 issue とし、本 issue では「受信しても接続を維持する」ことを目的とする。展開しない場合、ペイロードは読み捨てる。
- 予約型や未定義型の扱いも方針として明記する。

## 完了条件

- メッセージ型 16 / 19 / 22 のメッセージを受信しても `Error` にならず、接続が維持される。
- 無視したことがイベントで観測できる。
- 既存の対応済みメッセージ型の解析に回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `RtmpMessageType::from_type_id` に 16 / 19 / 22 を追加し、`RtmpMessage` で扱わない型として分類する。
2. チャンクデコーダまたはメッセージデコーダで、未処理型のペイロードを読み捨てて `MessageIgnored` を返す経路を追加する。
3. 型 22 のメッセージを受信する回帰テストを追加する。
