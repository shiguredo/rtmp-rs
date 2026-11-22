# RtmpUrl の未到達経路のテストを追加する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/test-rtmp-url-coverage
- Polished: {YYYY-MM-DD}

## 目的

`RtmpUrl` の解析の境界条件を回帰テストで守り、URL の不具合が混入しても検出できるようにする。

## 現状

- `src/rtmp_url.rs` の `RtmpUrl::parse_with_stream_name` はテストから呼ばれていない。
- ポート番号の `u16` 変換失敗（65536 以上、負数、非数字）は単体テストで到達していない。既存の `test_invalid_port` は数字以外の経路しか踏んでいない。
- ブラケット付き IPv6 の後置が不正な形式（`[::1]x` など）はテストされていない。
- `pbt/tests/prop_rtmp_url.rs` が存在せず、PBT で URL の解析と `Display` の往復を確認していない。
- そのため、URL 解析の変更が既存の境界条件を壊しても検出できない。

## 設計方針

- `tests/test_rtmp_url.rs` に公開 API の単体テストを追加する。private な関数は直接テストしない。
- 対象は `parse` と `parse_with_stream_name`、`Display`、`tls`、`host`、`port`、`app`、`stream_name`。
- ケースは次のとおり。
  - rtmp / rtmps、ポートあり / なし、IPv4、ホスト名、ブラケット付き IPv6
  - app / stream の空、末尾スラッシュ、複数スラッシュ
  - query / fragment を含む URL
  - ポート 0、65535、65536、負数、非数字
  - ブラケットなし IPv6、空のブラケット、ブラケット後の不正文字
  - `parse_with_stream_name` で stream 名を上書きするケース
- `pbt/tests/prop_rtmp_url.rs` に、`Display` した文字列を `parse` して元の URL と一致することを検証する PBT を追加する。生成器は有効な URL の範囲に限定する。
- モックやスタブは使わない。テストのログメッセージは日本語にする。

## 完了条件

- 上記の単体テストが追加され、境界条件が固定されている。
- `parse_with_stream_name` のテストが追加されている。
- `prop_rtmp_url.rs` の PBT が追加され、`Display` と `parse` の往復が検証されている。
- 既存の URL 解析の挙動に変更がない。

## 解決方法

1. `tests/test_rtmp_url.rs` を追加し、`parse` / `parse_with_stream_name` / `Display` のテストを書く。
2. `pbt/tests/prop_rtmp_url.rs` を追加し、`prop_rtmp_url` を PBT の実行対象に含める。
3. UI 表記のテストケース名は英語、コメントとログは日本語にする。
