# 変更履歴

- UPDATE
  - 後方互換がある変更
- ADD
  - 後方互換がある追加
- CHANGE
  - 後方互換のない変更
- FIX
  - バグ修正

## develop

- [CHANGE] MSRV (rust-version) を 1.95 に上げる
  - @voluntas

- [CHANGE] `encode_video_frame`、`RtmpMessageEncoder::encode`、`RtmpMessageChannel::feed_send_message` の戻り値を `Result<(), Error>` にする
  - @voluntas

- [CHANGE] `RtmpMessageType` に `SharedObjectAmf3`、`SharedObjectAmf0`、`Aggregate` を追加する。網羅的な `match` には分岐の追加が必要になる
  - @voluntas

- [CHANGE] 受信 ACK の値や欠落による RTMP 接続の自動切断を廃止し、相手の値を `RtmpConnectionEvent::AckReceived` で通知する
  - @voluntas

- [CHANGE] `RtmpMessageChannel` の `notify_ack_received` と `set_local_ack_window_size` を削除し、`advance_send_buf` の戻り値を `()` にする
  - @voluntas

- [ADD] RTMP の各要素をモジュールとして公開 API に追加する
  - @voluntas

- [UPDATE] `ErrorKind` から `#[non_exhaustive]` を削除する
  - @voluntas

- [FIX] サーバーが C2 を先に受信すると S2 の送信が止まる問題を修正する
  - @voluntas

- [FIX] RTMP URL のクエリやフラグメント内の `/` で app と stream が誤分割される問題を修正する
  - @voluntas

- [FIX] RTMP URL でブラケットなしの IPv6、不正なブラケット構造、ユーザー情報を拒否する
  - @voluntas

- [FIX] FLV の AVC 追加ヘッダーをコマンドフレームでも読み書きし、コーデックとパケットタイプが矛盾する送信を拒否する
  - @voluntas

- [FIX] FLV の AVC `CompositionTime` が符号付き 24 ビットの範囲外なら、切り詰めずに入力エラーを返す
  - @voluntas

- [FIX] Shared Object と Aggregate の型 ID を受理し、本文を読み飛ばして `MessageIgnored` を通知する
  - @voluntas

- [FIX] サーバーは初回の `connect` だけを受理し、分割受信中の ACK 送信で切断状態にならないようにする
  - @voluntas

### misc

- [UPDATE] PBT を proptest から noprop に移行する
  - @voluntas

- [UPDATE] テスト専用の `tests` モジュールを廃止し、テストを公開 API 経由に変更する
  - @voluntas

- [UPDATE] CI のツールチェーンを `rust-toolchain.toml` の MSRV に揃える
  - @voluntas
