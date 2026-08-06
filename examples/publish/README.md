# RTMP Publish Client Example

H.264/AAC の MP4 ファイルを RTMP サーバーに配信するサンプルです。

ローカルホスト（1935 番ポート）の RTMP サーバーへの配信は、以下のコマンドで実行できます。

```bash
# [NOTE]
# 配信先サーバーを変更したい場合には `cargo run -p publish -- -h` で
# 表示されるヘルプを参考にして、引数を指定してください
cargo run -p publish --app live --stream stream /path/to/input.mp4
```

配信先として利用可能な RTMP サーバーは次のコマンドで起動できます。

```bash
cargo run -p server
```

あるいは `ffplay` コマンドを以下のように起動すれば、配信されたストリームを直接表示することができます。

```bash
ffplay -listen 1 -i rtmp://127.0.0.1:1935/live/stream
```
