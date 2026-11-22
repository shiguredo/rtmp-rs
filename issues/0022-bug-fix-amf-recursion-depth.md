# AMF のデコードに再帰深さ制限を追加する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-amf-recursion-depth
- Polished: {YYYY-MM-DD}

## 目的

AMF0 / AMF3 のデコードに再帰深さの上限を設け、深いネストを含むメッセージでプロセスがスタックオーバーフローするのを防ぐ。

## 現状

- `src/amf0.rs` の `Decoder::decode_value` は、厳密配列 (`decode_strict_array`) やオブジェクトのメンバー (`decode_pairs`) を再帰的にデコードする。
- `src/amf3.rs` の `Decoder::decode_value` も、配列 (`decode_array`)、オブジェクト (`decode_object`)、キーと値の組 (`decode_pairs`) を再帰的にデコードする。
- どちらにも深さカウンタやスタック使用量の検証がなく、ネストの深さは入力に依存する。
- RTMP の 1 メッセージの上限は 16,777,215 バイトであり、その範囲内で十分深いネストを作れる。

### 再現条件

1. AMF3 の場合、`09 03 01` を 200,000 回繰り返し、最後に `01`（null）を付けた 600,001 バイトの入力を組み立てる。
2. `Amf3Value::decode` でデコードする。

期待は深さ上限超過として `ErrorKind::InvalidData` を返すこと。実際はスタックオーバーフローを起こし、`thread '...' has overflowed its stack` でプロセスが abort する。AMF0 でも `0A 00 00 00 01` の繰り返しと `05` で同様に再現できる。

## 設計方針

- AMF0 / AMF3 のデコーダに共通の深さカウンタを導入し、上限（例: 64〜128）を超えたら `ErrorKind::InvalidData` を返す。
- 上限値は定数として定義し、doc コメントに理由（スタック使用量の見積もり）を書く。
- デコードの再帰呼び出しのすべての経路（配列、オブジェクト、キーと値の組、AMF0 の AVM+ 経由の AMF3）でカウンタを増減させる。
- 上限ちょうどは受理し、上限 + 1 は拒否することをテストで固定する。
- 既存の PBT / fuzz は深いネストを生成しないため、この問題を検出できないことをテスト方針に明記する。

## 完了条件

- AMF3 の深さ 200,000 の入力がスタックオーバーフローせず、`ErrorKind::InvalidData` を返す。
- AMF0 でも同様に深さ上限を超える入力がエラーになる。
- 上限内のネストを持つ既存の AMF0 / AMF3 のデコードに回帰がない。
- 上限値と境界のテストが追加されている。

## 解決方法

1. `src/amf0.rs` と `src/amf3.rs` の `Decoder` に深さフィールドを追加する。
2. 複合型のデコード前に深さを検証し、上限超過時に `ErrorKind::InvalidData` を返す。
3. AMF0 と AMF3 の境界テストを追加する。
