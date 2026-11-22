# RtmpUrl の往復・明示ポート・ストリーム名の別指定をテストする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/test-rtmp-url-coverage
- Polished: 2026-11-22

## 目的

`RtmpUrl` の解析の境界条件を回帰テストで守り、URL の不具合が混入しても検出できるようにする。

## 現状

- `src/rtmp_url.rs` の単体テストは、両スキームの既定ポート、ホスト名・IPv4・ブラケット付き IPv6、空の app / stream、複数階層の app、固定文字列の `Display` をすでに確認している。
- `RtmpUrl::parse_with_stream_name` は `fuzz/fuzz_targets/fuzz_rtmp_url.rs` から呼ばれるが、app と stream の値を期待値に照らす単体テストや PBT はない。第一引数の URL のパス全体が app になり、既存のストリーム名を置き換える API ではない。
- 既存の `test_invalid_port` の入力はブラケットなしのホストと非数字のポートであり、数字検査で拒否される。ポート 0・65535 の受理と、65536 以上の `u16` 変換失敗を確認するテストはない。
- `pbt/tests/prop_rtmp_url.rs` はまだ存在しない。固定入力の文字列比較や、ファジング上の解析・表示の呼び出しはあるが、生成した複数のホストやポートで `Display` 後の再解析結果を比較する性質テストはない。

## 設計方針

- 公開の `RtmpUrl::parse`、`parse_with_stream_name`、`Display` を noprop の性質テストで検証する。既存の固定ケースと同じ単体テストは追加しない。URL 実装と公開 API に変更を加えない。
- 往復に使う入力は、query / fragment やユーザー情報を含まない、ホスト名・IPv4・ブラケット付き IPv6 とする。スキームは `rtmp` と `rtmps`、ポートは省略または `u16` の値、app は `/` でつながる 1 個以上の非空のセグメント、stream は `/`、`?`、`#` のない非空の 1 セグメントを使う。生成には英数字など区切りにならない文字を使う。
- 生成したホスト、ポート、app、stream、TLS フラグを独立した期待値とする。`parse` には app と stream を含む URL、`parse_with_stream_name` には app までの URL と別引数の stream を渡し、両方の全フィールドを期待値と比較する。続いて `Display` が明示ポート付きの `scheme://host:port/app/stream` を出力し、その結果を `parse` すると同じ 5 フィールドを得ることを確認する。省略ポートは `rtmp` が 1935、`rtmps` が 443 となるため、元の入力文字列との一致は要求しない。
- 明示ポートについて、有効なホスト名とブラケット付き IPv6 それぞれで 0 と 65535 を受理し、65536 と空の明示ポートは `ErrorKind::InvalidInput` になることを両解析 API で確認する。負数と非数字は、ブラケット付き IPv6 で `u16` 変換へ進んで拒否される場合を確認する。ブラケットなしホストの非数字を変換前に拒否する経路は既存テストが通るため、同じケースは再追加しない。接続先ポートとしての利用可否は検証しない。
- query / fragment と別引数に `/`、`?`、`#` を含む場合は 0035、ブラケット構造やユーザー情報が不正な場合は 0036 の回帰で扱い、ここでは重複させない。

## 完了条件

- `pbt/tests/prop_rtmp_url.rs` で、両スキーム・ホスト 3 形式・既定と明示ポートを含む生成値について、両解析 API と `Display` 後の再解析で 5 フィールドが一致する。
- query / fragment なしの `parse_with_stream_name` で、パスの最後のセグメントも app に残る。たとえば `rtmp://example.invalid/live/original` と `replacement` から `app = "live/original"`、`stream_name = "replacement"` を得る。明示ポートの境界は乱数の結果に依存せず、上記の値を検証する。
- 既存の URL テストと新しい PBT が通る。非公開関数は直接呼ばず、モック・スタブを使わない。コメントとテストログは日本語とする。

## 解決方法

1. `pbt/tests/prop_rtmp_url.rs` を作成する。0035 または 0036 が同じテストターゲットを先に作成していた場合はそこに追記する。既存の noprop テストと同じ方法で、生成値による往復と明示ポートの境界を確認する。
2. `cargo test -p pbt --test prop_rtmp_url` と既存の URL テストを実行する。CI の `cargo test --workspace` の対象に自動で含まれるため、テストターゲットの手動追加は不要。
