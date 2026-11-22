# RTMP URL の query / fragment 内のスラッシュで app / stream の分割が壊れる問題を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-rtmp-url-query-split
- Polished: {YYYY-MM-DD}

## 目的

RTMP URL の query や fragment にスラッシュが含まれていても、app と stream を正しく分割できるようにする。

## 現状

- `src/rtmp_url.rs` の `RtmpUrl::parse` は、`parse_scheme_and_host_port` が返したパス全体を `rsplit_once('/')` で分割し、query / fragment を区別しない。
- そのため、query の中のスラッシュが app と stream の境界として使われる。
- `...?token=ab/` のように query の末尾がスラッシュの場合、stream 名が空としてエラーになる。

### 再現条件

1. `RtmpUrl::parse("rtmp://example.com/live/stream?token=ab/cd")` を呼ぶ。
2. `app` と `stream_name` を確認する。

期待は app = `live`、stream = `stream?token=ab/cd`（または query を別フィールドで保持）。実際は app = `live/stream?token=ab`、stream = `cd` になる。

## 設計方針

- fragment（`#`）と query（`?`）をパスから分離してから、パスの最後の `/` で app と stream を分割する。
- query を `RtmpUrl` に保持するかを決める。既存の公開フィールド（`app`、`stream_name`）だけを維持する場合は、query を stream 名に含めるか、接続先として扱わないかを明記する。
- RFC 3986 の順序（path、query、fragment）に従い、fragment 以降は RTMP の接続先として扱わない。
- 既存の query を含まない URL の解析に回帰がないようにする。
- 認証トークンなどの値は issue やテストに実値を書かず、ダミー値を使う。

## 完了条件

- `rtmp://example.com/live/stream?token=ab/cd` の app と stream が意図どおりに分割される。
- query の末尾がスラッシュ、fragment を含む、query と fragment の両方を含むケースでパニックや空 stream エラーにならない。
- 既存の URL 解析に回帰がない。
- 上記を確認する単体テストと PBT を追加する。

## 解決方法

1. `src/rtmp_url.rs` でパス、query、fragment を分離する。
2. パスの最後の `/` で app と stream を分割する。
3. query / fragment の扱いを doc に明記し、テストを追加する。
