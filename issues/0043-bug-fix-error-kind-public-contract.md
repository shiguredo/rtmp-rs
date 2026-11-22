# `ErrorKind::InsufficientBuffer` の公開契約を明確にする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-error-kind-public-contract
- Polished: {YYYY-MM-DD}

## 目的

`ErrorKind` の公開 API としての契約を、ドキュメントと実装で一致させる。

## 現状

- `src/error.rs` の `ErrorKind::InsufficientBuffer` は `#[doc(hidden)]` が付いているが `pub` であり、外部から観測できる。`tests/test_amf.rs` でもこの variant を観測している。
- `ErrorKind` から `#[non_exhaustive]` が削除されているため、外部で網羅 `match` を書くと、新しい variant を知らない利用側はコンパイルエラーになる。
- `README.md` のエラー一覧には 4 種類しか記載されておらず、`InsufficientBuffer` の扱いが説明されていない。
- ドキュメント上は内部用に見えるが、実際には公開 API として振る舞っており、契約が不明確。

### 再現条件

1. 外部クレートから `shiguredo_rtmp::ErrorKind` の値を網羅 `match` する。
2. `InsufficientBuffer` の variant が存在するため、ドキュメントに記載のない variant を扱う必要がある。

期待は、`InsufficientBuffer` が公開 API として文書化されているか、公開 API から除去されていること。

## 設計方針

- `InsufficientBuffer` を公開 API として残す場合は、`#[doc(hidden)]` を外して意味と発生条件を文書化し、README のエラー一覧にも追加する。
- 内部専用にする場合は、`ErrorKind` から除去して内部の別型（`InternalError` など）へ移す。この場合は `Error` の公開表現と `Display` の互換性を確認する。
- どちらを選ぶ場合も、`ErrorKind` の網羅性に関する方針（`#[non_exhaustive]` の有無）を明確にする。
- 破壊的変更になる場合は CHANGES.md に CHANGE として記載する。

## 完了条件

- `InsufficientBuffer` の扱いが公開 API のドキュメントと README に明記されている、または公開 API から除去されている。
- 外部クレートからの網羅 `match` の挙動が意図どおりになる。
- 上記を確認するテストを追加する。

## 解決方法

1. `ErrorKind::InsufficientBuffer` を公開するか内部化するかを決定する。
2. `src/error.rs`、`README.md`、`CHANGES.md` を更新する。
3. `ErrorKind` を網羅 `match` するテストを追加し、公開契約を固定する。
