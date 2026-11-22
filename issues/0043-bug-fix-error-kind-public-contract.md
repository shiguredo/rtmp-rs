# `ErrorKind::InsufficientBuffer` の公開契約を明確にする

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-error-kind-public-contract
- Polished: 2026-11-22

## 目的

`ErrorKind` の公開 API としての契約を、ドキュメントと実装で一致させる。

## 現状

- `src/error.rs` の `ErrorKind::InsufficientBuffer` は `#[doc(hidden)]` が付いているが `pub` であり、外部から観測できる。`tests/test_amf.rs` でもこの variant を観測している。
- `ErrorKind` は `2539282` で `#[non_exhaustive]` が削除され、現行の 5 バリアントを列挙すれば外部クレートでも網羅的な `match` を書ける。README に掲載された 4 種だけを分岐に列挙した `match` は網羅的でない。
- `README.md` のエラー一覧には 4 種類しか記載されておらず、`InsufficientBuffer` の扱いが説明されていない。
- ドキュメント上は内部用に見えるが、実際には公開 API として振る舞っており、契約が不明確。

### 再現条件

外部クレートで `shiguredo_rtmp::ErrorKind` を README にある 4 種だけで `match` すると、`InsufficientBuffer` の分岐がないためコンパイルできない。既存の 5 種をすべて分岐に含めるとコンパイルできるが、その 5 種目は現行の公開 API ドキュメントと README の一覧にはない。

## 設計方針

- `ErrorKind::InsufficientBuffer` は既存の公開バリアントとして維持し、`#[doc(hidden)]` と内部用とするコメントを取り除く。公開列挙型は現行どおり網羅可能なままとし、プロジェクトの Rust 規約に従い `#[non_exhaustive]` を付けない。列挙値・シグネチャー・`Error` の構造・表示・エラー分類は変更しない。
- ドキュメントでは、入力から必要なバイト数を読み取れなかったことを示す種別と定義し、追加受信で回復できるかは API の契約によると記載する。たとえば AMF のスライスデコーダーでは、入力が途中なら `Err` の種別として観測できる。`RtmpMessageDecoder::decode` が `Ok(None)` にするのはチャンク入力の不足だけであり、完成済みペイロードの解析エラーはそのまま返される。完成済みペイロードのエラーから追加入力が必要だとは判断しない。
- 本 issue は公開済みの契約を文書化する。User Control の完成済みペイロードの分類変更は 0042 の対象であり、本 issue では変更しない。既存の挙動を変えない文書化のみなので変更履歴のエントリは追加しない。

## 完了条件

- rustdoc の `ErrorKind` と README のエラー一覧に 5 種が表示され、必要バイト数の不足を表すことと、呼び出す API や不足が発生した段階により、`Err` の `kind` が `InsufficientBuffer` になる場合と `Ok(None)` になる場合があることを説明している。
- rustdoc の例が、外部クレートとしてワイルドカードなしで 5 種を参照・分岐できることを確認する。AMF の入力不足とチャンク入力不足の挙動は既存テストで確認し、挙動を再検証する重複テストは加えない。表示確認は生成したドキュメントで行う。

## 解決方法

1. `src/error.rs` の `ErrorKind::InsufficientBuffer` から `#[doc(hidden)]` を外して意味を説明し、`ErrorKind` に網羅例を追加する。
2. `README.md` に 5 種目と、追加の入力を待つべきかどうかを各 API の契約で判断することを明記する。
3. rustdoc の生成結果、doc テスト、既存の AMF とメッセージデコーダーのテストを確認する。
