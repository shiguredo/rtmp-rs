# RTMP URL の query / fragment 内のスラッシュで app / stream の分割が壊れる問題を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-rtmp-url-query-split
- Polished: 2026-11-22

## 目的

RTMP URL の query や fragment にスラッシュが含まれていても、app と stream を正しく分割できるようにする。

## 現状

- `src/rtmp_url.rs` の `RtmpUrl::parse` は、`parse_scheme_and_host_port` が返したパス全体を `rsplit_once('/')` で分割し、query / fragment を区別しない。
- そのため、query や fragment の中の `/` が app と stream の境界として使われる。`...?token=ab/` のように query の末尾が `/` の場合、stream 名が空だと判定される。
- `RtmpUrl::parse_with_stream_name` は同じヘルパーが返した文字列をそのまま `app` にするため、URL の query と fragment は `app` に入る。別引数の `stream_name` は加工しない。

### 再現条件

1. `RtmpUrl::parse("rtmp://example.com/live/stream?token=ab/cd")` を呼ぶ。
2. `app` と `stream_name` を確認する。

期待値は `app = "live"`、`stream_name = "stream?token=ab/cd"`。現行実装では `app = "live/stream?token=ab"`、`stream_name = "cd"` になる。

## 設計方針

- [RFC 3986 §3.2–3.5](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2) に従い、URL 文字列全体から最初の `#` 以降を fragment として除外し、その手前にある最初の `?` から query を分離する。残った URL で authority とパスを分けるため、query や fragment 内の `/` は authority の終端にもパスの区切りにもならない。パーセントデコードは行わない。
- 両 API とも、authority とパスの境界にある最初の `/` は名前に含めない。`parse` とそれに委譲する `FromStr` は、残りのパスの最後の `/` だけで app と stream を分割する。パス由来の app と stream が非空であることを確認してから、query があれば区切りの `?` ごと未加工で `stream_name` に付ける。fragment がなく query に `/` を含まない従来の有効な入力と同様、query は publish / play のストリーム名に含め、connect の `app` と `tcUrl` には入れない。
- `parse_with_stream_name` は最初の `/` を除いた第一引数のパス全体を app とし、非空であることを確認してから、その URL の query があれば `?` ごと未加工で `app` に付ける。これにより connect の `app` と `tcUrl` に query を渡す既存の挙動を維持する。別引数は URL ではないので、非空であることだけを確認し、`/`、`?`、`#` を含んでいてもそのまま `stream_name` にする。
- 公開フィールド、`Display` の整形処理、RTMP コマンドの生成処理は変更しない。`parse` した値の `Display` には保持した query が残り、URL の fragment は出力されない。`parse_with_stream_name` で app に query を保持した値について、`Display` と `parse` の往復は保証しない。
- query も fragment もない URL の解析は維持する。ホストやポートの検証規則の変更は 0036 の対象とする。
- 認証トークンなどの値は issue やテストに実値を書かず、ダミー値を使う。

## 完了条件

- 再現例が期待値どおりに解析され、`Display` は `rtmp://example.com:1935/live/stream?token=ab/cd` を返す。
- 有効なパスに対し、query の末尾が `/` でもその値を保持する。fragment のみ、または query と fragment の両方がある場合は、fragment 内に `/` や `?` があっても app と stream の分割に影響せず、URL の fragment は保持しない。
- `parse_with_stream_name` に `rtmp://example.com/live?token=ab/cd#section/a` と `stream?x/y#z` を渡すと、`app = "live?token=ab/cd"`、`stream_name = "stream?x/y#z"` になる。
- URL のパスが欠落している場合や、各 API がパスから必要とする名前が空の場合は、query や fragment に文字があっても `ErrorKind::InvalidInput` を返す。別引数の空の `stream_name` も引き続き拒否する。
- 公開 API の PBT で、query / fragment の内容にかかわらず、パスから決まる境界、名前の非空条件と上記の保持規則が変わらないことを確認する。単体テストを追加するのは PBT で実現できないケースに限る。
- query / fragment がある URL の挙動と、別引数に `/`、`?`、`#` を含む例の回帰は本 issue が担当する。0045 では、query / fragment がない URL の一般的な往復検証と、本 issue の回帰に含まれない既存の境界条件を扱い、同じケースを再追加しない。

## 解決方法

1. `src/rtmp_url.rs` の `RtmpUrl::parse` と `RtmpUrl::parse_with_stream_name` が共有する URL の分離処理で、authority とパスを切り出す前に query / fragment の境界を確定する。
2. 各 API でパスから必要な名前を取り出して検証した後、設計方針に従って query を保持する。
3. 各解析 API の doc コメントに query / fragment の扱いを明記し、完了条件の回帰テストを追加する。
