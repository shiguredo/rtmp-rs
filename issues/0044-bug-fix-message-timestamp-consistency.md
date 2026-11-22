# 音声 / 映像メッセージのヘッダータイムスタンプとフレームのタイムスタンプの二重管理を解消する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-message-timestamp-consistency
- Polished: {YYYY-MM-DD}

## 目的

`RtmpMessage::Audio` / `RtmpMessage::Video` が同じ意味のタイムスタンプを 2 か所に持つ状態を解消し、公開 API で構築したメッセージの往復で値が変わらないようにする。

## 現状

- `src/rtmp_message.rs` の `RtmpMessage::Audio { header, frame }` と `Video { header, frame }` は、
  メッセージヘッダーの `RtmpMessageHeader::timestamp` と `AudioFrame::timestamp` / `VideoFrame::timestamp` を別々に持つ。
- `src/rtmp_message_encoder.rs` の `encode_message_payload` は、チャンクのタイムスタンプに `message.header().timestamp` を使い、フレーム側の `timestamp` は使わない。
- `src/rtmp_message_decoder.rs` は、受信したチャンクのタイムスタンプをフレームの `timestamp` に設定する。
- `RtmpPublishClientConnection::send_audio` / `send_video` は両者を一致させているため、通常の送信経路では問題が露呈しない。

### 再現条件

1. `RtmpMessage::Audio` を `header.timestamp = ZERO`、`frame.timestamp = RtmpTimestamp::from_millis(1000)` で構築する。
2. エンコードしてデコードする。

期待は `frame.timestamp` が 1000 のまま。実際は 0 になる。

## 設計方針

- タイムスタンプの正本を 1 つに決める。候補は次のとおり。
  - フレームのタイムスタンプを正本とし、メッセージヘッダーのタイムスタンプを構築時に導出する。
  - メッセージヘッダーのタイムスタンプを正本とし、フレームのタイムスタンプを削除または無視する。
  - 両方を保持するが、エンコード時に不一致をエラーにする。
- 公開 API の破壊的変更を伴う場合は、影響範囲と移行方法を設計方針に明記し、CHANGES.md に CHANGE として記載する。
- 既存の `send_audio` / `send_video` の挙動に回帰がないようにする。

## 完了条件

- 公開 API で構築した音声 / 映像メッセージが、エンコードとデコードの往復で同じタイムスタンプを持つ。
- 不一致を許容する設計を選ぶ場合は、その条件と挙動がドキュメント化されている。
- 既存の送信経路に回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. タイムスタンプの正本を決定する。
2. `RtmpMessage`、エンコーダ、デコーダを正本に合わせて修正する。
3. 不一致ケースのテストと、通常経路の往復テストを追加する。
4. 必要に応じて CHANGES.md を更新する。
