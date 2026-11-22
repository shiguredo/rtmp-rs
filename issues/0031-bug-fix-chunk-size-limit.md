# 仕様上有効なチャンクサイズを拒否する上限を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-chunk-size-limit
- Polished: {YYYY-MM-DD}

## 目的

RTMP 仕様で有効なチャンクサイズを通知するピアを接続断にせず、Set Chunk Size メッセージを仕様に沿って受理する。

## 現状

- `src/rtmp_chunk.rs` の `RtmpChunkSize::MAX` は 65536 であり、`RtmpChunkSize::new` は 1〜65536 の値だけを受理する。
- `src/rtmp_message_decoder.rs` の `chunk_to_message` は、Set Chunk Size メッセージの値を `RtmpChunkSize::new` で検証し、範囲外を `invalid chunk size` の `ErrorKind::InvalidData` にしている。
- RTMP 仕様 5.4.1 節は、チャンクサイズを 1〜0x7FFFFFFF と定義している。65537 以上を通知する仕様準拠のピアを、この実装は接続エラーにする。

### 再現条件

1. Set Chunk Size メッセージ（メッセージ型 1）のペイロードに 100000（0x000186A0）を設定する。
2. `RtmpMessageDecoder::decode` を呼ぶ。

期待はチャンクサイズ 100000 として受理すること。実際は `invalid chunk size: 100000` のエラーになる。

## 設計方針

- `RtmpChunkSize` の上限を仕様の 0x7FFFFFFF まで広げる。
- 巨大なチャンクサイズを受信した場合のメモリ消費とバッファ管理への影響を設計方針に明記する。チャンクサイズは受信ペイロードの分割単位であり、受信バッファをチャンクサイズ分だけ先に確保する実装は行わない。
- 実装上の上限を残す必要がある場合は、`ErrorKind::InvalidData` ではなく「認識したが未対応」を表す `ErrorKind::Unsupported` に分類し、理由をエラーメッセージに含める。
- 送信側の `RtmpConnectionOptions::chunk_size` の既定値と `RtmpChunkSize::saturating_new` の挙動も整合させる。

## 完了条件

- 65536 / 65537 / 100000 / 0x7FFFFFFF の Set Chunk Size が仕様どおり受理される。
- 0 や 0x80000000 以上の値は引き続きエラーになる。
- 受信したチャンクサイズで分割されたメッセージを正しく組み立てられる。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `RtmpChunkSize::MAX` を 0x7FFFFFFF に変更する。
2. `chunk_to_message` の Set Chunk Size 検証とエラー分類を更新する。
3. 境界値と、変更後のチャンクサイズで分割受信するテストを追加する。
