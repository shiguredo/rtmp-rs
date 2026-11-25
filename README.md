# rtmp-rs

[![crates.io](https://img.shields.io/crates/v/shiguredo_rtmp.svg)](https://crates.io/crates/shiguredo_rtmp)
[![docs.rs](https://docs.rs/shiguredo_rtmp/badge.svg)](https://docs.rs/shiguredo_rtmp)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![GitHub Actions](https://github.com/shiguredo/rtmp-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/shiguredo/rtmp-rs/actions/workflows/ci.yml)
[![Discord](https://img.shields.io/badge/Discord-%235865F2.svg?logo=discord&logoColor=white)](https://discord.gg/shiguredo)

## About Shiguredo's open source software

We will not respond to PRs or issues that have not been discussed on Discord. Also, Discord is only available in Japanese.

Please read <https://github.com/shiguredo/oss> before use.

## 時雨堂のオープンソースソフトウェアについて

利用前に <https://github.com/shiguredo/oss> をお読みください。

## 概要

Rust で実装された依存 0 かつ `no_std` に対応した Sans I/O な RTMP ライブラリです。

## 特徴

- Sans I/O
- 依存ライブラリ 0
- `no_std` 対応

## できること

- RTMP サーバーに接続して、映像と音声を配信する
- RTMP サーバーに接続して、配信中の映像と音声を受け取る
- RTMP サーバーを実装して、配信や再生を受け付ける
- RTMP のメッセージや AMF0 / AMF3、FLV の音声 / 映像タグを変換する

## 使い方

このライブラリはソケット通信を行いません。
TCP / TLS の接続は利用側で用意し、受信したバイト列を接続オブジェクトに渡し、接続オブジェクトが送りたいバイト列をソケットに書きます。

```rust
// connection は配信 / 再生 / サーバーの接続オブジェクト
// socket は TCP / TLS のソケット
// buf は受信用のバッファ

// 配信なら Publishing (配信中)、再生なら Playing (再生中) を目標にする
let target_state = RtmpConnectionState::Publishing;

// 目標の状態になるまで、送信と受信を繰り返す
loop {
    // 1. 接続が送りたいデータをソケットに書く
    let data = connection.send_buf();
    socket.write_all(data)?;
    connection.advance_send_buf(data.len());

    // 2. ソケットから読んだデータを接続に渡す
    //    buf は受信用のバッファ
    let n = socket.read(&mut buf)?;
    if n == 0 {
        // 接続が切れた
        break;
    }
    connection.feed_recv_buf(&buf[..n])?;

    // 3. 接続に起きた出来事を処理する
    while let Some(_event) = connection.next_event() {
        // イベントの種類に応じて処理する
    }

    // 4. 目標の状態になったら接続完了
    if connection.state() == target_state {
        break;
    }
}
```

### 配信する

RTMP サーバーに接続して、映像と音声を配信できます。
接続が完了すると、映像フレームと音声フレームを送信できます。

```rust
use shiguredo_rtmp::{RtmpPublishClientConnection, RtmpUrl};

// 配信先の URL を解析する
let url = RtmpUrl::parse("rtmp://localhost:1935/live/stream")?;

// 配信クライアントを作る
let mut connection = RtmpPublishClientConnection::new(url);

// 上のループで Publishing (配信中) まで進める
// (配信クライアントの場合は target_state を Publishing にする)

// 映像フレームを送る
// タイムスタンプ、フレームタイプ、コーデック、データを指定する
// connection.send_video(video_frame)?;

// 音声フレームを送る
// タイムスタンプ、音声フォーマット、サンプルレート、データを指定する
// connection.send_audio(audio_frame)?;
```

### 再生する

RTMP サーバーに接続して、配信中の映像と音声を受け取れます。
接続が完了すると、音声フレームと映像フレームがイベントとして届きます。

```rust
use shiguredo_rtmp::{RtmpConnectionEvent, RtmpPlayClientConnection, RtmpUrl};

// 再生先の URL を解析する
let url = RtmpUrl::parse("rtmp://localhost:1935/live/stream")?;

// 再生クライアントを作る
let mut connection = RtmpPlayClientConnection::new(url);

// 上のループで Playing (再生中) まで進める
// (再生クライアントの場合は target_state を Playing にする)

// 届いたイベントを処理する
while let Some(event) = connection.next_event() {
    match event {
        RtmpConnectionEvent::VideoReceived(frame) => {
            // 映像フレームをデコードや描画に渡す
        }
        RtmpConnectionEvent::AudioReceived(frame) => {
            // 音声フレームを再生に渡す
        }
        _ => {
            // 接続状態の変化など
        }
    }
}
```

### サーバーを実装する

RTMP サーバーを実装できます。
1 つの接続ごとにサーバー接続を作り、配信要求や再生要求を許可または拒否します。

```rust
use shiguredo_rtmp::{RtmpConnectionEvent, RtmpServerConnection};

// クライアント 1 つ分のサーバー接続を作る
let mut connection = RtmpServerConnection::new();

// ソケットから読んだデータを接続に渡す
connection.feed_recv_buf(&buf[..n])?;

// 接続に起きた出来事を処理する
while let Some(event) = connection.next_event() {
    match event {
        RtmpConnectionEvent::PublishRequested { .. } => {
            // 配信を許可する
            connection.accept()?;
            // 拒否する場合は理由を付けて reject する
            // connection.reject("already publishing")?;
        }
        RtmpConnectionEvent::PlayRequested { .. } => {
            // 再生を許可する
            connection.accept()?;
        }
        RtmpConnectionEvent::VideoReceived(_frame) => {
            // 配信された映像を、再生中の接続へ転送する
        }
        RtmpConnectionEvent::AudioReceived(_frame) => {
            // 配信された音声を、再生中の接続へ転送する
        }
        _ => {
            // 接続状態の変化など
        }
    }
}

// 接続が送りたいデータをソケットに書く
let data = connection.send_buf();
socket.write_all(data)?;
connection.advance_send_buf(data.len());

// 実際は、受信、イベント処理、送信を繰り返す
```

### エラー

処理に失敗するとエラーが返ります。

```rust
// 入力が途中で終わった場合
// AmfValue::decode は InsufficientBuffer (入力不足) を返すことがある
match AmfValue::decode(input, AmfVersion::Amf0) {
    Ok((consumed, value)) => {
        // 値が取り出せた
    }
    Err(error) if error.kind == ErrorKind::InsufficientBuffer => {
        // 続きのバイト列を加えて再試行する
    }
    Err(error) => {
        // 入力が不正
    }
}
```

```rust
// チャンクからメッセージを取り出す場合
match decoder.decode() {
    Ok(Some(message)) => {
        // メッセージが揃った
    }
    Ok(None) => {
        // 入力が足りない。続きのバイト列を追加して再試行する
    }
    Err(error) => {
        // 解析に失敗した
        // 宣言長まで受信済みのメッセージの解析失敗は、追加受信では補えない
    }
}
```

## 公開 API

詳しい API は [docs.rs](https://docs.rs/shiguredo_rtmp) を参照してください。

## サンプル

publish と server のサンプルは [examples/](https://github.com/shiguredo/rtmp-rs/tree/develop/examples) にあります。

## ライセンス

Apache License 2.0

```text
Copyright 2026 Shiguredo Inc.
Copyright 2026 Takeru Ohta (Original Author)

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
