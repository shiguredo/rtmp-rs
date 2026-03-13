# RTMP ライブラリ WebAssembly API

`c-api` クレートを wasm32 向けにビルドし、WebAssembly から使いやすい追加関数を提供します。

## ビルド方法

```bash
rustup target add wasm32-unknown-unknown
cargo build -p wasm --target wasm32-unknown-unknown --profile release-wasm
```

- 出力ファイル: `target/wasm32-unknown-unknown/release-wasm/rtmp_wasm.wasm`

## 提供する関数

### メモリ管理

- `rtmp_alloc`
- `rtmp_free`
- `rtmp_vec_ptr`
- `rtmp_vec_len`
- `rtmp_vec_free`

### JSON 変換

- `rtmp_audio_frame_from_json`
- `rtmp_audio_frame_to_json`
- `rtmp_video_frame_from_json`
- `rtmp_video_frame_to_json`
- `rtmp_connection_event_to_json`

### c-api の関数

`c-api` が提供する `rtmp_publish_client_connection_*` などの関数も、そのまま利用できます。

## テスト

```bash
cargo test -p wasm --quiet
```
