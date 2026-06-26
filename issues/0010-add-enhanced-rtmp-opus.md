# Enhanced RTMP の Opus 配信/受信対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-opus
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Enhanced Audio 仕様 (FourCC `Opus`) に対応し、Opus を含む音声フレームを送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリの音声コーデックカバレッジを拡張し、Sora など時雨堂製品との連携や、Opus 配信をサポートする主要配信ツールとの相互接続を可能にする。

## 優先度根拠

High。Opus は時雨堂製品 (Sora 等) で広く採用されている音声コーデックであり、eRTMP 経由で Opus を扱えることは本ライブラリの主要ユースケースに直結する。
また、eRTMP v2 仕様で MultichannelConfig との連携を明示的に規定された数少ないコーデックの 1 つで、完全準拠の観点からも優先実装が必要。

## 現状

- ExAudioTagHeader 基盤は issue 0009 (Enhanced Audio 基盤) で導入される予定であり、本 issue はその基盤の上に Opus のペイロード処理を追加するスコープ
- 既存の `AudioFormat` enum (`src/media.rs:144`) には Opus のエントリが無く、Opus 用 Sequence Header に相当する構造体も存在しない

## 設計方針

- Enhanced RTMP v2 の Enhanced Audio セクション (`docs/enhanced/enhanced-rtmp-v2.md` の Opus 関連記述、1009/1031 行付近) に準拠する
- 0009 (Enhanced Audio 基盤) で整備される ExAudioTagHeader 基盤の上に、Opus 用の分岐とペイロード処理を追加する
- Opus 用 Sequence Header は OpusSequenceHeader (RFC 7845 Section 5.1 の ID header) として表現する。`from_bytes` / `to_bytes` を提供する
- SequenceStart のペイロードが空の場合は、AudioPacketType.MultichannelConfig 経由でチャネルマッピングを取得する経路を許容する (空でない場合は ID header 内の情報を優先)
- `CodedFrames` における Opus ペイロードは「N ストリーム分の Opus パケット列」として扱う。最初の (N-1) パケットは RFC 6716 Appendix B の self-delimiting framing、最終パケットは RFC 6716 Section 3 の undelimited framing

## 完了条件

- SoundFormat=`ExHeader` で AudioFourCc が `Opus` の AudioTagHeader をデコード/エンコードできる
- AudioPacketType の `SequenceStart` / `CodedFrames` / `SequenceEnd` を Opus ペイロードと正しく組み合わせて扱える
- OpusSequenceHeader (ID header) のパース/シリアライズが round-trip する
- 空ペイロードの SequenceStart + MultichannelConfig 連携が動作する
- ユニットテストで Opus の round-trip が確認できる (シングルストリーム/マルチストリーム両方)
- pbt の Opus 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている
- 既存の legacy / 0009 で導入された他コーデックの動作に回帰が無い

## 解決方法

1. `src/media.rs`
   - `AudioFourCc` enum (0009 で導入) の `Opus` バリアントを使う
   - `OpusSequenceHeader` (RFC 7845 ID header) を新規構造体として追加し、`from_bytes` / `to_bytes` を実装する
2. `src/flv.rs`
   - ExAudioTagHeader 経路で AudioFourCc が `Opus` の場合のペイロード分岐を追加する
   - SequenceStart 空ペイロード + MultichannelConfig の連携を扱う
3. `src/lib.rs`
   - `OpusSequenceHeader` 等の新規型を re-export する
4. `tests/`
   - Opus の round-trip テストを追加する (ID header あり/なし、シングル/マルチストリーム)
5. `pbt/`
   - Opus 用 Generator を追加する
6. `fuzz/`
   - `OpusSequenceHeader::from_bytes` と Opus AudioFrame デコードの fuzz ターゲットを追加する
