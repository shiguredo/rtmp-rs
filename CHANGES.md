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

- [ADD] RTMP の各要素をモジュールとして公開 API に追加する
  - @voluntas

- [UPDATE] `ErrorKind` から `#[non_exhaustive]` を削除する
  - @voluntas

- [FIX] RTMP ACK の計数起点の違いで誤切断される問題を修正する
  - @voluntas

### misc

- [UPDATE] PBT を proptest から noprop に移行する
  - @voluntas

- [UPDATE] テスト専用の `tests` モジュールを廃止し、テストを公開 API 経由に変更する
  - @voluntas

- [UPDATE] CI のツールチェーンを `rust-toolchain.toml` の MSRV に揃える
  - @voluntas
