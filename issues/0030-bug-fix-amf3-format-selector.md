# AMF3 メッセージの Enhanced RTMP フォーマットセレクタに対応する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-amf3-format-selector
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の AMF3 メッセージ形式に従い、AMF3 の Data / Command メッセージを準拠ピアと相互接続できるようにする。

## 現状

- `refs/enhanced-rtmp-v2.md` の「Enabling AMF3 in RTMP」では、メッセージ型 15 / 16 / 17 のペイロード先頭はフォーマットセレクタバイトで、現在定義されているのは 0（AMF0 文脈）のみとされている。AMF3 の値は AMF0 の AVM+ marker（0x11）を前置する。
- `src/rtmp_message_encoder.rs` の `RtmpMessageEncoder::encode_command` と `encode_data_payload` は、`AmfVersion::Amf3` でもセレクタを出力せず、純粋な AMF3 値をそのまま書く。
- `src/rtmp_message_decoder.rs` は DataAmf3（型 15）の先頭バイトをセレクタとして扱わず、そのまま AMF3 の型マーカーとして読む。CommandAmf3（型 17）だけは先頭 0x00 を AMF0 文脈として扱う部分対応がある。

### 再現条件

1. `RtmpMessage::Data { amf_version: AmfVersion::Amf3, values: vec![AmfValue::Amf3(Amf3Value::String("x"))] }` を `RtmpMessageEncoder::encode` でエンコードする。
2. メッセージのペイロード先頭を確認する。

期待はフォーマットセレクタ 0x00 で始まり、AMF3 値が 0x11 を伴うこと。実際は AMF3 の文字列マーカー 0x06 で始まる。

## 設計方針

- メッセージ型 15 / 16 / 17 のエンコード時に、ペイロード先頭へフォーマットセレクタ 0x00 を出力する。
- AMF3 の値は AMF0 の AVM+ marker 0x11 の後に AMF3 の値を出力する。AMF0 文脈の値はそのまま AMF0 で出力する。
- デコード時は先頭のフォーマットセレクタを読み、0 なら AMF0 文脈として解釈する。未定義のセレクタ値は `ErrorKind::Unsupported` とする。
- 既存の AMF0 メッセージ（型 18 / 20）のワイヤ形式と、CommandAmf3 の既存の 0x00 処理に回帰がないようにする。
- `AmfVersion` の意味を「ワイヤ上のセレクタと文脈」と整合させる。

## 完了条件

- AMF3 の Data メッセージのペイロードが 0x00 で始まり、AMF3 の値が 0x11 を伴って出力される。
- フォーマットセレクタ 0x00 + AMF0 値の DataAmf3 / CommandAmf3 をデコードできる。
- 未定義のセレクタ値は `ErrorKind::Unsupported` になる。
- AMF0 の Data / Command メッセージの往復に回帰がない。
- 上記を確認する単体テストと、既存 PBT の往復テストを追加または修正する。

## 解決方法

1. `RtmpMessageEncoder` の Data / Command エンコードで、AMF3 のときセレクタと AVM+ marker を出力する。
2. `RtmpMessageDecoder` の DataAmf3 / CommandAmf3 でセレクタを読み、AMF0 文脈と AMF3 値を切り替える。
3. `tests/test_rtmp_message_decoder.rs` と PBT にセレクタ付きデータのケースを追加する。
