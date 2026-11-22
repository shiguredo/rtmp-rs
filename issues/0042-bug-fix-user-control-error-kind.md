# 完全受信済みの短い UserControl payload を InvalidData として扱う

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-user-control-error-kind
- Polished: {YYYY-MM-DD}

## 目的

メッセージとして完全に受信済みで、内容が不正な UserControl イベントを `ErrorKind::InsufficientBuffer` ではなく `ErrorKind::InvalidData` として扱い、呼び出し側が「データ待ち」と「不正データ」を取り違えないようにする。

## 現状

- `src/rtmp_user_control_event.rs` の `RtmpUserControlEvent::decode` は `read_u16` / `read_u32` を順に呼ぶ。イベント型が既知でペイロードが短い場合、これらの読み取りが `ErrorKind::InsufficientBuffer` を返す。
- このエラーは `src/rtmp_message_decoder.rs` の `chunk_to_message` からそのまま伝播する。
- `RtmpMessageDecoder::decode` が `ErrorKind::InsufficientBuffer` を `Ok(None)` に変換するのは、チャンクデコーダ自体がデータ不足を返した場合だけである。メッセージ本体のデコードエラーはそのまま返る。
- メッセージはチャンクとして完全に受信済みであり、データが後から増えることはない。

### 再現条件

1. `RtmpUserControlEvent::decode(&[0x00, 0x00, 0x00])` を呼ぶ。
2. 返る `ErrorKind` を確認する。

期待は `ErrorKind::InvalidData`。実際は `ErrorKind::InsufficientBuffer` になる。

## 設計方針

- チャンクデコーダ由来の `InsufficientBuffer`（まだバイト列が揃っていない）と、メッセージペイロードの解析で発生した `InsufficientBuffer`（完全受信済みだが内容が不正）を区別する。
- メッセージデコーダの `chunk_to_message` で、各イベントのデコード結果が `InsufficientBuffer` の場合は `InvalidData` に変換する。または `RtmpUserControlEvent::decode` の先頭でイベント型ごとの必要長を検証して `InvalidData` を返す。
- Set Chunk Size、Abort、Ack、WinAckSize、Set Peer Bandwidth などの固定長制御メッセージにも同じ扱いを適用するか検討する。
- 公開 API の挙動変更になるため、CHANGES.md への記載を検討する。

## 完了条件

- 短い UserControl payload が `ErrorKind::InvalidData` になる。
- データ不足でチャンクが未完の場合は従来どおり `Ok(None)` になる。
- 制御メッセージのエラー種別が一貫している。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `RtmpUserControlEvent::decode` でイベント型ごとの必要バイト数を検証し、不足なら `ErrorKind::InvalidData` を返す。
2. 必要に応じて `RtmpMessageDecoder::chunk_to_message` で `InsufficientBuffer` を `InvalidData` に正規化する。
3. 短い payload のテストを追加する。
