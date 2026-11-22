# メッセージ型 16 / 19 / 22 を未知型エラーにせず受信する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-message-type-coverage
- Polished: 2026-11-22

## 目的

RTMP で定義されている型 16 / 19 / 22 のメッセージを、内容には対応しないまま受信できるようにする。型だけを理由に受信 API がエラーを返さず、無視した型を通知して後続のメッセージを処理できるようにする。

## 現状

- `src/rtmp_message.rs` の `RtmpMessageType::from_type_id` は 1、2、3、4、5、6、8、9、15、17、18、20 のみを受理し、それ以外を `unknown message type` の `ErrorKind::InvalidData` にする。
- [Adobe RTMP 仕様（2012 年 12 月版）§7.1.3](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=24) は Shared Object に 16（AMF3）と 19（AMF0）を、[同仕様 §7.1.6](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=26) は Aggregate に 22 を割り当てている。
- チャンクデコーダは `decode_message_header` で `RtmpMessageType::from_type_id` を呼ぶため、これらの型を受信するとメッセージヘッダーの解析時点でエラーになる。

### 再現条件

1. 新しい `RtmpMessageDecoder` の `feed_buf` に、fmt 0、チャンクストリーム ID 3、タイムスタンプ 0、メッセージ型 22、メッセージストリーム ID 1、ペイロード長 0 のチャンクを渡す。ヘッダー段階の失敗を切り分けるための入力であり、Aggregate ボディの例ではない。
2. `decode` を呼ぶ。現行実装では `ErrorKind::InvalidData` と理由 `unknown message type: 22` を返す。型 ID を 16 または 19 に置き換えても同様に失敗する。

## 設計方針

- `RtmpMessageType` に `SharedObjectAmf3 = 16`、`SharedObjectAmf0 = 19`、`Aggregate = 22` を追加し、`from_type_id` が認識するようにする。既存の判別値、数値への変換、その他の予約・未定義 ID を `ErrorKind::InvalidData` にする挙動は変更しない。公開列挙型へのバリアント追加により、利用側の網羅的な `match` には対応が必要になる。
- これら 3 型も宣言された長さどおりに通常のチャンク組み立てを行う。メッセージが完成したら `RtmpMessageDecoder` の内部受信結果に種別と共通ヘッダーだけを残し、ボディは解釈せず破棄する。Shared Object の AMF や形式セレクターの解析、Aggregate のサブメッセージの展開、`RtmpMessage` を使った送信は対象外とする。
- `RtmpMessage` と公開の `RtmpMessageDecoder::decode`、`RtmpMessageChannel::next_recv_message` の戻り値は変えない。両公開メソッドは無視対象を飛ばし、すでに受信済みの後続メッセージがあれば同じ呼び出しで返す。無視対象を消費しただけで `Ok(None)` または `None` を返して後続の処理を止めない。
- 接続が使うチャネル内部では、通常メッセージと無視対象を同じ受信キューで完成順に渡す。クライアントとサーバーは無視対象 1 件ごとに既存の `RtmpConnectionEvent::MessageIgnored` を 1 件追加する。イベントの `name` は種別名、`detail` はメッセージストリーム ID とタイムスタンプで特定できるようにし、ボディは含めない。これらの型だけを理由に接続状態を変えない。チャンク処理のエラー、圧縮ヘッダーとタイムスタンプの履歴、ACK の受信バイト計数は従来どおりとする。ハンドシェイク完了と同じ入力で受信した残余を処理するタイミングは 0026 の対象とし、本 issue では変更しない。

## 完了条件

- `RtmpMessageDecoder::decode` と `RtmpMessageChannel::next_recv_message` は 16 / 19 / 22 を飛ばし、同じ入力に続く対応済みメッセージを追加の受信なしで返す。対応外の予約・未定義 ID は引き続きデコード時に `ErrorKind::InvalidData` になる。
- ハンドシェイク完了とは別の受信呼び出しで、クライアントとサーバーの公開 API から 3 型それぞれで `MessageIgnored` が 1 件観測できる。ペイロードが複数のチャンクや入力に分かれても完成前には通知せず、完成後は同じチャンクストリームの fmt 1 ヘッダーで続く対応済みメッセージも処理できる。イベントにボディを含めず、無視対象の処理だけでは接続状態を変更しない。
- 既存の対応済み型の回帰がなく、3 型の後続処理・通知・分割入力を公開 API の性質テストで確認する。単体テストの追加は PBT で実現できないケースに限る。

## 解決方法

1. `src/rtmp_message.rs` の `RtmpMessageType` と `from_type_id` に 3 型を追加する。公開バリアントの説明は doc コメント（`///`）に、型 ID の仕様根拠は資料名・節番号と将来の仕様変更の可能性を含めて実装コメント（`//`）に記載する。
2. `RtmpMessageDecoder` と `RtmpMessageChannel` に内部受信結果の経路を追加し、低レベルの公開 API では通知を返さず、接続側だけが既存のイベントに変換する。
3. メッセージデコーダーとクライアント・サーバーの受信経路の回帰テストを更新する。
