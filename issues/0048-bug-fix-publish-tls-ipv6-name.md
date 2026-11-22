# 配信サンプルで IPv6 ホストを TLS のサーバー名に変換する

- Created: 2026-11-23
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-publish-tls-ipv6-name
- Polished: {YYYY-MM-DD}

## 目的

配信サンプルで、ブラケット付き IPv6 の RTMPS URL が TLS のサーバー名への変換で拒否されないようにする。

## 現状

配信サンプルの `connect_tls` は、`RtmpUrl::host` をそのまま `ServerName::try_from` に渡す。URL から解析した IPv6 ホストはブラケットを保持するため、有効な IPv6 でも DNS 名にも IP アドレスにも変換できず、ソケット接続前にエラーになる。ソケットに渡す接続先文字列では、IPv6 とポートの境界を区別するためブラケットが必要である。

## 設計方針

TLS の `ServerName` に変換する箇所でだけ、ブラケット付き IPv6 を IP アドレスとして扱う。ソケット向けの表記、DNS 名と IPv4 の変換、証明書検証、ライブラリの `RtmpUrl` の表記は変更しない。

## 完了条件

- ブラケット付き IPv6 を `ServerName::IpAddress` に変換できる。無効な IPv6 は DNS 名へ置き換えて受理しない。
- DNS 名と IPv4 の従来の挙動を維持する。
- TLS 名の変換を、外部ネットワーク接続を伴わないテストで確認する。
