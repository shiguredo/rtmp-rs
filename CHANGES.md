# CHANGES

## Unreleased

### Added

- `crates/c-api` を追加し、RTMP の publish / play / server 接続、音声 / 映像フレーム、イベント、エラーを C FFI として利用できるようにした
- `crates/wasm` を追加し、wasm 向けのメモリ管理関数と、音声 / 映像フレームおよび接続イベントの JSON 変換 API を追加した
- ルート workspace に `c-api` / `wasm` を組み込み、`release-wasm` プロファイルを追加した
- README と各クレート README を追加 / 更新し、C API / wasm API の使い方を追記した
