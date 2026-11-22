# 音声・映像メッセージのヘッダーとフレームのタイムスタンプの不一致を拒否する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-message-timestamp-consistency
- Polished: 2026-11-22

## 目的

`RtmpMessage::Audio` と `RtmpMessage::Video` は公開型を維持したまま、ヘッダーとフレームのタイムスタンプの不一致を送信前に検出する。一方の値を黙って捨てず、一致するメッセージは従来どおりの時刻で往復できるようにする。

## 現状

- `src/rtmp_message.rs` の `RtmpMessage::Audio { header, frame }` と `Video { header, frame }` は、
  メッセージヘッダーの `RtmpMessageHeader::timestamp` と `AudioFrame::timestamp` / `VideoFrame::timestamp` を別々に持つ。
- `src/rtmp_message_encoder.rs` の `RtmpMessageEncoder::message_to_chunk` は、チャンクのタイムスタンプに `message.header().timestamp` を使う。FLV のフレームエンコーダーは、フレーム側の絶対タイムスタンプをペイロードに書かない。
- `src/rtmp_message_decoder.rs` の `chunk_to_message` は、受信したチャンクからメッセージヘッダーを作り、同じ時刻を FLV デコーダーへ渡してフレームのタイムスタンプにも設定する。
- `RtmpPublishClientConnection` と `RtmpServerConnection` の `send_audio` / `send_video` はフレーム側の時刻をヘッダーへコピーするため、これらの公開送信メソッドでは両方の値が最初から一致する。

### 再現条件

音声メッセージを `header.timestamp = RtmpTimestamp::ZERO`、`frame.timestamp = RtmpTimestamp::from_millis(1000)` とし、ほかのフィールドは有効な値で構築する。現行実装でエンコードしてデコードすると、ヘッダーとフレームの時刻は両方とも 0 ミリ秒になる。修正後の期待値は、エンコード時に `ErrorKind::InvalidInput` を返すこと。

## 設計方針

- [Adobe RTMP 仕様 §4](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=5) と [§5.1](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=6) のメッセージタイムスタンプは 32 ビットのミリ秒値 1 つである。公開の `RtmpMessage`、`RtmpMessageHeader`、`AudioFrame`、`VideoFrame` の構造は変えない。本ライブラリの送信契約として、音声と映像ではヘッダー側とフレーム側の時刻の完全一致を要求する。一致した場合は従来どおりヘッダーの値をワイヤに使う。
- 0037 を先に実装し、そこで使う `RtmpMessageEncoder::encode` と `RtmpMessageChannel::feed_send_message` の `Result<(), Error>` を再利用する。0024 で同じ経路が先に `Result` 化されていれば、それを使う。音声・映像ともにエンコード境界で不一致を検出し、ペイロードを作る前に `ErrorKind::InvalidInput` を返す。失敗時は既存の出力バッファとチャンクエンコーダー状態を変更しない。チャネルは既存のエラー伝播経路で呼び出し元へ返す。
- タイムスタンプ同士の比較だけを追加する。受信側、FLV の `CompositionTime`、チャンク時刻の差分やラップ処理、音声・映像以外のメッセージの処理は変更しない。
- `RtmpMessage` の該当バリアントと、エンコーダー・チャネルの公開 API の doc コメントに一致条件とエラーを記載する。既存利用者は、直接メッセージを構築するときにフレームの時刻をヘッダーへコピーするか、両方に同じ時刻を設定する。これまで成功した不一致の入力が拒否されるため、実装時に `CHANGES.md` の `develop` に `[CHANGE]` を記載する。

## 完了条件

- 音声・映像ともに、不一致の入力は `ErrorKind::InvalidInput` になる。既存の出力バイト列は変わらない。同一チャンクストリームで有効な先行メッセージをエンコードしてから不一致を試し、後続の有効なメッセージの出力が、不一致の呼び出しを省いた場合と同じであることを確認する。
- 一致する音声・映像は、エンコードとデコードの往復で元の時刻をヘッダーとフレームの両方に保持する。対応するチャンクストリーム ID へ変換できるメッセージストリーム ID を使った場合は、チャネル経由でも時刻の不一致を同じエラー種別で返す。ストリーム ID の変換自体は 0034 の対象とする。配信クライアントの配信中とサーバーの再生中に `send_audio` / `send_video` を使い、実際に送信バッファへ追加された音声・映像メッセージをデコードして、ヘッダーとフレームの時刻が入力フレームの値と一致することを確認する。
- `prop_rtmp_message` と `prop_rtmp_connection` の公開 API を使う PBT に拒否・出力不変・後続処理の検証を加え、既存の正常往復を維持する。接続の送出時刻は `prop_rtmp_client_connection` と `prop_rtmp_server_connection` の既存送信試験を拡張して確認する。単体テストは PBT で実現できない確認に限る。

## 解決方法

1. `RtmpMessageEncoder::encode` の副作用より前に音声・映像のタイムスタンプの一致検査を追加する。
2. 公開 API の説明、エンコーダーとチャネルの回帰テスト、変更履歴を更新する。
