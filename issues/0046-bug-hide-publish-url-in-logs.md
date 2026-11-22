# 配信サンプルの起動ログに RTMP URL 全体を出力しないようにする

- Created: 2026-11-23
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-hide-publish-url-in-logs
- Polished: {YYYY-MM-DD}

## 目的

RTMP URL のクエリやストリーム名に認証情報が含まれる場合でも、配信サンプルの起動ログに残らないようにする。

## 現状

配信サンプルの `main` は、配信処理の開始前に `RtmpUrl` を `Display` で整形して標準出力へ書き出す。整形結果には保持した `app` と `stream_name` が含まれ、クエリも省略されない。

## 設計方針

配信サンプルの起動時に URL 全体を表示しない。接続に使用する値と、ライブラリの `RtmpUrl` の解析・整形契約は変更しない。

## 完了条件

- 起動時の出力に入力由来の URL 全体、`app`、`stream_name`、クエリが含まれない。
- 機密情報ではない合成入力を使い、外部接続を必要とせずに起動時の出力を検証する。
