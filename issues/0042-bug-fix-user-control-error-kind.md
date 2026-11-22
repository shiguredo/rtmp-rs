# 完成済みの User Control ペイロードの長さ不足を InvalidData として扱う

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-user-control-error-kind
- Polished: 2026-11-22

## 目的

完成済みの User Control ペイロードに必要なバイトが足りない場合は `ErrorKind::InvalidData` とし、追加受信を待てば完成するチャンクと区別する。

## 現状

- `src/rtmp_user_control_event.rs` の `RtmpUserControlEvent::decode` は、イベント型を `read_u16` で、その後の必要なフィールドを `read_u32` で読む。イベント型を読み切れない場合や、既知型で必要なイベントデータが足りない場合は `ErrorKind::InsufficientBuffer` を返す。イベント型を読み取れても未知の値なら `ErrorKind::InvalidData` になる。
- このエラーは `src/rtmp_message_decoder.rs` の `chunk_to_message` からそのまま伝播する。
- `RtmpMessageDecoder::decode` が `ErrorKind::InsufficientBuffer` を `Ok(None)` に変換するのは、チャンクデコーダ自体がデータ不足を返した場合だけである。メッセージ本体のデコードエラーはそのまま返る。
- `chunk_to_message` はチャンクデコーダーから宣言長ぶんのペイロードを受け取った後で呼ばれる。その段階では、後続の入力が届いても当該ペイロードは増えない。

### 再現条件

`RtmpUserControlEvent::decode(&[0x00, 0x00, 0x00])` を呼ぶ。先頭 2 バイトは `StreamBegin`（型 0）で、4 バイト必要なストリーム ID が 1 バイトしかない。期待値は `ErrorKind::InvalidData` だが、現行実装は `ErrorKind::InsufficientBuffer` を返す。

## 設計方針

- `RtmpUserControlEvent::decode` は渡されたスライス全体を 1 件の完成済みペイロードとして扱う。イベント型または後続フィールドの読み取りが `InsufficientBuffer` になった場合だけ、この公開 API の境界で `InvalidData` に変換する。既存の読み取り手順から必要長を判定し、イベント型と長さの対応表を別に持たない。
- [Adobe RTMP 仕様 §6.2](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=23) はイベント型を先頭 2 バイトとし、[同 §7.1.7](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=27) はイベントごとのデータを定義している。本ライブラリが現に扱う型の最小ペイロード長は `SetBufferLength` が 10 バイト、ほかの既知型は 6 バイト。後者には、同節に列挙されていない既存実装の `BufferEmpty` と `BufferReady` も含む。既存どおり、必要なフィールドより後のバイトは無視し、未知のイベント型は `InvalidData` のままとする。
- `RtmpMessageDecoder` は完成したメッセージに対する `InvalidData` をそのまま返す。チャンク入力が足りない場合の `Ok(None)` は変更しない。ほかの制御メッセージやペイロードの分類変更、`ErrorKind::InsufficientBuffer` 自体の可視性と残る利用箇所の契約（0043）は対象外とする。
- `RtmpUserControlEvent::decode` のエラー条件を公開 API の doc コメントに明記し、公開 API で観測できるエラー種別の変更を実装時に `CHANGES.md` の `develop` の `[CHANGE]` に記載する。

## 完了条件

- `RtmpUserControlEvent::decode` が、イベント型を読み切れない入力と、全既知型それぞれの必要長に満たない入力を `InvalidData` にする。既知型の正常入力と余分なバイトを伴う場合の挙動は変わらない。未知のイベント型の既存テストも通る。
- 公開 `RtmpMessageDecoder::decode` で、有効なチャンクヘッダーを持つ User Control メッセージが宣言長まで届いていなければ `Ok(None)`、宣言長まで届いていてもイベントの必要長に満たなければ `Err` の `kind` が `InvalidData` になる。正しいイベントを分割して受信した場合は、全データを揃えれば正常にデコードされる。
- 上記を公開 API の PBT で確認する。既存テストと重複する単体テストは追加せず、PBT で実現できない確認に限って単体テストを追加する。

## 解決方法

1. `src/rtmp_user_control_event.rs` の公開デコード境界にエラーの変換を置き、イベントのフィールド読み取りと未知型の判定は既存の処理を使う。
2. `prop_rtmp_user_control_event` と `prop_rtmp_message` に不足の境界と分割受信の検証を加える。チャンクデコーダーとほかのメッセージ種別の処理は変更しない。
