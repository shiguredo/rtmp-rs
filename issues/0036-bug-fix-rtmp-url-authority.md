# RTMP URL の authority（IPv6 とユーザー情報）の解釈を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-rtmp-url-authority
- Polished: {YYYY-MM-DD}

## 目的

`RtmpUrl::parse` が authority を RFC 3986 に沿って解釈し、IPv6 アドレスやユーザー情報を含む URL で接続先を誤らないようにする。

## 現状

- `src/rtmp_url.rs` の `parse_host_port` は、ブラケットで囲まれていない authority について「最後の `:` の後ろが数字だけならポート番号」と解釈する。
- そのため `rtmp://2001:db8::1/live/stream` は host = `2001:db8:`、port = 1 になる。IPv6 アドレスは RFC 3986 ではブラケットで囲む必要がある。
- `rtmp://[]/live/stream` のような空のブラケットも受理される。
- `rtmp://user@example.com/live/stream` は host に `@` を含んだまま受理され、`rtmp://user:pass@example.com/live/stream` はポート番号の解析でエラーになる。ユーザー情報の扱いが一貫していない。

### 再現条件

1. `RtmpUrl::parse("rtmp://2001:db8::1/live/stream")` を呼ぶ。
2. `host` と `port` を確認する。

期待は、ブラケットなしの IPv6 を拒否するか、host = `2001:db8::1` として扱うこと。実際は host = `2001:db8:`、port = 1 になる。

`RtmpUrl::parse("rtmp://user@example.com/live/stream")` では host に `user@example.com` が入る。

## 設計方針

- authority を RFC 3986 の `[userinfo "@"] host [":" port]` として解析する。
- IPv6 アドレスはブラケット必須とし、ブラケットなしの IPv6 は `ErrorKind::InvalidInput` にする。ブラケット付きの場合は host にブラケットを含めるか外すかを現行仕様から決め、`Display` と往復できるようにする。
- ユーザー情報は、URL に保持するのか拒否するのかを決める。保持しない場合は `@` を含む authority を一貫して拒否する。
- ポート番号の u16 変換失敗は従来どおり `ErrorKind::InvalidInput` とする。
- 既存の `rtmp://host:1935/app/stream` と `rtmp://[::1]:1935/app/stream` の挙動は維持する。

## 完了条件

- ブラケットなし IPv6、空のブラケット、ユーザー情報付き URL の扱いが一貫し、テストで固定される。
- ブラケット付き IPv6 の host と port が正しく解析され、`Display` と往復する。
- 既存の IPv4 / ホスト名 / ポート省略の解析に回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `src/rtmp_url.rs` に authority の解析処理を切り出し、userinfo、host、port を分離する。
2. IPv6 はブラケットの有無を検証し、不正形式を `ErrorKind::InvalidInput` にする。
3. ユーザー情報の扱いを決めて実装し、`Display` の出力と一致させる。
4. IPv4、ホスト名、IPv6、userinfo、不正形式のテストを追加する。
