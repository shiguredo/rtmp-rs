# RTMP Server Example

クライアントからの RTMP 配信および視聴（再生）要求を処理する RTMP サーバーのサンプルです。

ローカルホスト（1935 番ポート）で動作する RTMP サーバーは以下のコマンドで起動できます。

```bash
# [NOTE]
# 使用するポート番号などを変更したい場合には `cargo run -p server -- -h` で
# 表示されるヘルプを参考にして、引数を指定してください
cargo run -p server
```

この RTMP サーバーへのストリームの配信は以下のコマンドで行えます。
なお、入力に使用するファイルのコーデックは H.264 / AAC である必要があります。
```bash
# ffmpeg を使う場合
ffmpeg -re -i /path/to/input.mp4 -c:v copy -c:a copy -f flv rtmp://127.0.0.1:1935/live/stream

# この crate の publish サンプルを使う場合
cargo run -p publish --app live --stream stream /path/to/input.mp4
```

上で配信されたストリームの視聴は以下のコマンドで行えます。
```bash
ffplay rtmp://127.0.0.1:1935/live/stream
```

