# RTMP URL の authority（IPv6 とユーザー情報）の解釈を修正する

- Created: 2026-11-22
- Completed: 2026-11-23
- Branch: feature/fix-rtmp-url-authority
- Polished: 2026-11-22

## 目的

`RtmpUrl` の authority 解析で、ブラケットなしの IPv6 を別の `host` と `port` に誤分割せず、空のブラケットや未対応のユーザー情報も誤って受理しないようにする。

## 現状

- `src/rtmp_url.rs` の `parse_host_port` は、ブラケットで囲まれていない authority について「最後の `:` の後ろが数字だけならポート番号」と解釈する。
- そのため `rtmp://2001:db8::1/live/stream` は host = `2001:db8:`、port = 1 になる。IPv6 アドレスは RFC 3986 ではブラケットで囲む必要がある。
- `rtmp://[]/live/stream` のような空のブラケットも受理される。
- `rtmp://user@example.com/live/stream` は `host` に `@` を含んだまま受理される。一方、`rtmp://user:dummy@example.com/live/stream` は `:` の後ろが数字ではないため、ポートへの変換前に拒否される。ユーザー情報としては認識されない。
- ブラケット内の文字列が IPv6 アドレスかどうかは検証しない。既存の `test_invalid_ipv6_address` はこの挙動を固定している。

### 再現条件

`RtmpUrl::parse("rtmp://2001:db8::1/live/stream")` は `ErrorKind::InvalidInput` を返すべきだが、現行実装では `host = "2001:db8:"`、`port = 1` で成功する。

## 設計方針

- [RFC 3986 §3.2–3.2.2](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2) の authority 構文 `[userinfo "@"] host [":" port]` と IPv6 リテラルのブラケット規則を境界判定の根拠とする。変更対象は `RtmpUrl::parse` と `RtmpUrl::parse_with_stream_name` が使う `parse_host_port`。`FromStr` は従来どおり `parse` に委譲する。URL 全体から authority を切り出す境界は 0035 が扱う。
- ユーザー情報はサポートしない。authority に生の `@` が含まれていたら `ErrorKind::InvalidInput` を返し、値を保存せず、エラーメッセージにも含めない。パーセントデコードは行わない。
- ブラケットなしの authority では `:` をポートの区切りにだけ使い、複数の `:` やブラケット文字を含む場合は拒否する。ブラケット付きの形式では、先頭の `[` と対応する `]` の間は空でなく、他のブラケットを含まないことを確認する。`]` の後ろは何もないか `:port` の形式だけを許可する。これらを満たさない場合は `ErrorKind::InvalidInput` とする。ブラケット内のアドレス自体の妥当性やホスト名の書式を検証する変更は行わない。
- 空のホストは従来どおり拒否する。明示ポートの受理範囲は変更せず、ブラケットなしでは ASCII 数字だけかを確認してから、ブラケット付きではポート部分をそのまま `u16` に変換する。変換失敗は `ErrorKind::InvalidInput` とする。[RFC 3986 §3.2.3](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.2.3) の `port = *DIGIT` は空のポートも表せるが、空の明示ポートは従来どおり拒否する。ポート自体を省略した場合は `rtmp` が 1935、`rtmps` が 443 のままとする。
- ブラケット付きホストの `RtmpUrl::host` はブラケットを含む元の表記を保持する。公開フィールド、`Display`、`tcUrl` の生成は変更しない。変更は URL の解析に限定し、ソケット接続や TLS のサーバー名への変換は対象外とする。

## 完了条件

- ブラケットなし IPv6、`[]`、閉じブラケットの欠落、ブラケット後の不正文字、利用者名だけまたはパスワード付きのユーザー情報を含む authority は、両解析 API で `ErrorKind::InvalidInput` になる。
- `rtmp://host:1935/app/stream` と `rtmp://[::1]:1935/app/stream` ではそれぞれ従来どおり `host = "host"` または `host = "[::1]"`、`port = 1935` を得る。query も fragment もないブラケット付き IPv6 の URL を `parse` した値は、`Display` の出力を再度 `parse` して同じ値になる。ポートを省略した元の文字列と出力の一致は要求しない。
- 既存の IPv4、ホスト名、既定ポートおよび `test_invalid_ipv6_address` の挙動を維持する。今回追加する拒否条件は公開 API の PBT で回帰を検証し、単体テストの追加は PBT で実現できないケースだけに限る。
- 本 issue が追加するブラケット構造とユーザー情報の拒否の回帰は 0045 で重ねて追加しない。既存テストにない明示ポートの境界や一般的な往復検証は 0045 で扱う。

## 解決方法

- 両解析 API が使う `parse_host_port` は、生の `@` を含む authority を入力値のないエラーで拒否する。ブラケットの構造を検証し、ブラケットなしでは最初の `:` をポート区切りとして扱うことで、複数の `:` を拒否する。
- ブラケット付きホストの表記、明示ポートの変換、既定ポートの従来の挙動を維持した。`RtmpUrl` の文書に受理条件を記載した。ホストの中身の検証、公開フィールドの定義、整形と接続コマンドの生成は変更していない。
- `prop_rtmp_url` に、構造の拒否、ユーザー情報の非出力、ホスト表記とポートを検証する性質テストを追加した。既存の URL 単体テストも通過した。
