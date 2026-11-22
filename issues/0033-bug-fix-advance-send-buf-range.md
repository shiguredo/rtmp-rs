# `advance_send_buf` に送信バッファ長を超える値を渡したときのパニックを防ぐ

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-advance-send-buf-range
- Polished: {YYYY-MM-DD}

## 目的

送信バッファ長を超える送信済みバイト数を渡してもパニックせず、安全に扱えるようにする。

## 現状

- `src/rtmp_connection.rs` の `RtmpMessageChannel::advance_send_buf` は、引数 `n` をクランプせずに `Buf::advance` に渡す。
- `src/bytes.rs` の `Buf::advance` は `offset` を加算するだけで、`offset == bytes.len()` 以外ではバッファをクリアしない。
- `Buf::get` は `&self.bytes[self.offset..]` を返すため、`offset` が `bytes.len()` を超えると範囲外スライスでパニックする。
- ハンドシェイク側の `advance_send_buf` は `n.min(self.send_buf.len())` でクランプしており、メッセージチャネル側と挙動が非対称である。
- 非同期 I/O で、送信バッファの長さが変わった後に古い長さで `advance_send_buf` を呼ぶような順序でも発生し得る。

### 再現条件

1. `RtmpMessageChannel` にメッセージを 1 つ積み、`send_buf().len()` を取得する。
2. `advance_send_buf(len + 1)` を呼ぶ。
3. その後に `send_buf()` を呼ぶ。

期待はバッファ全体が消費されるか、エラーになること。実際は `range start index ... out of range for slice of length ...` でパニックする。

## 設計方針

- `RtmpMessageChannel::advance_send_buf` で `n` を送信バッファの残り長にクランプする。ハンドシェイク側と同じ扱いにそろえる。
- クランプではなくエラーを返す設計にする場合は、戻り値を `Result` に変更し、呼び出し側（クライアント / サーバー接続）で切断状態へ遷移させる。
- `Buf::advance` の doc に「呼び出し側が範囲を保証する」とあるが、公開 API から到達できる以上、呼び出し側で検証する。
- 送信バッファ長を超える値が渡されたことを観測できるようにするかは設計方針で決める。

## 完了条件

- `send_buf().len() + 1` を `advance_send_buf` に渡した後に `send_buf()` を呼んでもパニックしない。
- 通常の送信量の反映に回帰がない。
- 上記を確認する公開 API の回帰テストを追加する。

## 解決方法

1. `RtmpMessageChannel::advance_send_buf` の先頭で `n` をクランプする、または戻り値を `Result` にして範囲外をエラーにする。
2. クライアント / サーバー接続の `advance_send_buf` の扱いを更新する。
3. 範囲外の値を使う回帰テストを追加する。
