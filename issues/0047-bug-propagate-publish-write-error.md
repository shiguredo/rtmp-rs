# 配信サンプルのソケット書き込み失敗を呼び出し元に返す

- Created: 2026-11-23
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-propagate-publish-write-error
- Polished: {YYYY-MM-DD}

## 目的

配信サンプルが送信失敗を隠さず、配信を続けられない理由を呼び出し元に伝えられるようにする。

## 現状

配信サンプルの `run_publishing_loop` は `RtmpStream::write_all` の結果を `.ok()` で捨て、全量を送信したかどうかにかかわらず `advance_send_buf` に送信予定の長さを渡す。途中で失敗した場合も、未送信データを処理済みとして扱う。

## 設計方針

`write_all` のエラーを `run_publishing_loop` から返し、その場合は `advance_send_buf` を呼び出さない。追加の再送処理は導入せず、成功時の流れと公開 API は変更しない。

## 完了条件

- 書き込みに失敗した場合は元の I/O エラーを呼び出し元に返し、以後の配信処理を続けない。
- 正常時は全量の書き込みに成功してから、送信バッファをその長さだけ進める。
- 書き込みの成功と失敗を実 I/O で検証し、モックやスタブは使わない。
