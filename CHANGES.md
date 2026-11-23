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

- [ADD] RTMP の各要素をモジュールとして公開 API に追加する
  - @voluntas

- [UPDATE] `ErrorKind` から `#[non_exhaustive]` を削除する
  - @voluntas

- [FIX] RTMP ACK の計数起点の違いで誤切断される問題を修正する
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

### misc

- [UPDATE] PBT を proptest から noprop に移行する
  - @voluntas

- [UPDATE] テスト専用の `tests` モジュールを廃止し、テストを公開 API 経由に変更する
  - @voluntas

- [UPDATE] CI のツールチェーンを `rust-toolchain.toml` の MSRV に揃える
  - @voluntas
