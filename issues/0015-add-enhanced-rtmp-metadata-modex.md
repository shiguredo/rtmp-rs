# Enhanced RTMP の Metadata Frame (colorInfo/HDR) と ModEx (TimestampOffsetNano) 対応を追加する

- Priority: Medium
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-metadata-modex
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Metadata Frame 仕様 (`VideoPacketType.Metadata` + `colorInfo` HDR メタデータ) と、ExVideoTagHeader / ExAudioTagHeader 内の ModEx (TimestampOffsetNano によるナノ秒精度の timestamp 補正) に対応する。
これにより、eRTMP 完全準拠を目指す本ライブラリで HDR (HDR10 / HDR10+ / HLG / DV) 配信のメタデータ伝達と、サブミリ秒精度のタイミング同期を可能にする。

## 優先度根拠

Medium。eRTMP v2 仕様で明示的に定義される機能であり、完全準拠の観点から実装は必要である。
仕様の `CapsExMask.ModEx` / `CapsExMask.TimestampNanoOffset` / `SUPPORT_VID_CLIENT_HDR` / `SUPPORT_VID_CLIENT_VIDEO_PACKET_TYPE_METADATA` フラグでクライアント能力として明示的に宣言される。
Sora など時雨堂製品での直接的な需要は現時点で限定的なため Medium とする。

## 現状

- 0004 (HEVC) で `VideoPacketType.Metadata` および ModEx は `Error::unsupported` を返す方針となっている
- 0009 (Enhanced Audio 基盤) でも `AudioPacketType.ModEx` は `Error::unsupported` を返す方針となっている
- `colorInfo` を表現するための型 (ColorInfo / ColorConfig / HdrCll / HdrMdcv) はコードベースに存在しない
- ナノ秒オフセットを格納するための型 (videoTimestampNanoOffset / audioTimestampNanoOffset) も存在しない

## 設計方針

- Enhanced RTMP v2 の Metadata Frame セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 1446 行以降) と ExVideoTagHeader / ExAudioTagHeader 内 ModEx 記述 (1167, 783 行付近) に準拠する
- 本 issue では以下の 2 機能をまとめてスコープに含める (両者は ExVideoTagHeader / ExAudioTagHeader 内で隣接する別 PacketType として定義されており、実装上同じ箇所を触るため)
  - `VideoPacketType.Metadata` の AMF-encoded colorInfo パース/シリアライズ
  - ExVideoTagHeader / ExAudioTagHeader の ModEx ループと TimestampOffsetNano の扱い
- `ColorInfo` 構造体 (colorConfig / hdrCll / hdrMdcv の 3 サブ構造体) を定義し、AMF 経由のシリアライズ/デシリアライズを実装する
- AMF0 のみ対応 (AMF3 対応は別 issue)。AMF3 encoding 切替時は AMF0 経路から `Error::unsupported` を返す
- `VideoPacketModExType.TimestampOffsetNano` / `AudioPacketModExType.TimestampOffsetNano` を扱い、20 bit の UI24 ns 値をパースする
- ModEx のループは、複数の ModEx パケットが連続する可能性に対応する
- ColorInfo の `Undefined` 値による colorInfo リセットを表現する手段を用意する
- HDR formats (HDR10 / HDR10+ / HLG / DV) 自体の bitstream 解釈は本 issue のスコープ外 (codec bitstream 内の SEI/NALU/OBU は各コーデックの対応 issue で扱う)

## 完了条件

- `VideoPacketType.Metadata` を含む VideoFrame をデコード/エンコードできる
- `ColorInfo` (colorConfig / hdrCll / hdrMdcv) を AMF0 で round-trip できる
- colorInfo の `Undefined` 値 / 空オブジェクトによるリセットを正しく扱える
- ExVideoTagHeader / ExAudioTagHeader の ModEx ループを正しくパースできる
- `TimestampOffsetNano` (UI24 ns 値) をパース/シリアライズできる
- ユニットテストで以下が確認できる
  - ColorInfo の round-trip
  - ModEx (TimestampOffsetNano) の round-trip (映像/音声)
  - 複数 ModEx パケット連続のパース
- pbt の Metadata / ModEx 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている (Metadata / ModEx を含む VideoFrame / AudioFrame デコード)
- 既存の Enhanced Video / Enhanced Audio コーデック対応に回帰が無い

## 解決方法

1. `src/media.rs`
   - `ColorInfo` / `ColorConfig` / `HdrCll` / `HdrMdcv` 構造体を定義する
   - `VideoPacketModExType` / `AudioPacketModExType` enum を定義する
   - VideoFrame / AudioFrame にナノ秒オフセットや colorInfo を保持するフィールドを追加する、またはイベント列として返す方式とするかを実装段階で決定する
2. `src/flv.rs`
   - ExVideoTagHeader / ExAudioTagHeader の ModEx ループを実装する
   - VideoPacketType.Metadata の AMF-encoded colorInfo を読み書きする
3. `src/amf0.rs`
   - ColorInfo のシリアライズ/デシリアライズに必要な AMF0 サポートを確認する
4. `src/lib.rs`
   - `ColorInfo` 等の新規型を re-export する
5. `tests/`
   - ColorInfo の round-trip テストを追加する
   - ModEx の round-trip テストを追加する
6. `pbt/`
   - ColorInfo / ModEx 用 Generator を追加する
7. `fuzz/`
   - Metadata / ModEx を含む VideoFrame / AudioFrame デコードの fuzz ターゲットを追加する
