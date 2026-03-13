# RTMP ライブラリ C API

`shiguredo_rtmp` の Rust API を C から利用するための FFI レイヤです。

## ビルド方法

```bash
cargo build -p c-api
```

- ヘッダーファイル: `crates/c-api/include/rtmp.h`
- 静的ライブラリ: `target/debug/librtmp.a`

## 提供する主な関数

- `rtmp_library_version`
- `rtmp_publish_client_connection_*`
- `rtmp_play_client_connection_*`
- `rtmp_server_connection_*`
- `rtmp_audio_frame_*`
- `rtmp_video_frame_*`
- `rtmp_connection_event_*`
- `rtmp_last_error_message`

## テスト

```bash
cargo test -p c-api --quiet
```

`crates/c-api/tests/simple_connection.c` を C コンパイラでビルドして実行します。
