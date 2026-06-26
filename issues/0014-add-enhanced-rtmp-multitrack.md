# Enhanced RTMP の Multitrack (映像/音声) 対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-multitrack
- Polished: {YYYY-MM-DD}

## 目的

Enhanced RTMP の Multitrack Streaming 仕様 (映像/音声) に対応し、1 つの video / audio message 内に複数トラック (異なるビットレート/解像度/コーデック/言語/カメラアングル) を多重化して送受信できるようにする。
これにより、eRTMP 完全準拠を目指す本ライブラリで ABR ladder 配信、複数言語音声、同期マルチカメラ等のユースケースをサポートできるようになる。

## 優先度根拠

High。eRTMP の主要な拡張機能の 1 つであり、Sora の SFU 配信ユースケース (1 接続で複数視点/品質) と直結する。
仕様の `CapsExMask.Multitrack` フラグでクライアント能力として明示的に宣言される機能であり、eRTMP 完全準拠を掲げる以上、対応必須。

## 現状

- 0004-0008 (Enhanced Video) および 0009-0013 (Enhanced Audio) で `VideoPacketType.Multitrack` / `AudioPacketType.Multitrack` の受信時には `Error::unsupported` を返す方針となっている
- 既存の VideoFrame / AudioFrame 構造体は単一トラック前提であり、trackId / sizeOfTrack の概念が存在しない

## 設計方針

- Enhanced RTMP v2 の Multitrack Streaming セクション (`docs/enhanced/enhanced-rtmp-v2.md` の 1611 行以降) および ExVideoTagHeader / ExAudioTagHeader の Multitrack 分岐 (1216, 823 行付近) に準拠する
- VideoPacketType.Multitrack / AudioPacketType.Multitrack の経路を実装する
- `AvMultitrackType` (OneTrack / ManyTracks / ManyTracksManyCodecs) を扱う
- trackId (UI8) と sizeOfTrack (UI24) のパース/シリアライズを実装する。OneTrack の場合は sizeOfTrack を省略する
- 1 つの video / audio message 内で複数トラックを連続パースする (positionDataPtrToNextTrack 相当のロジック)
- 「ManyTracksManyCodecs では各トラック直前で FourCC を読み直す」「OneTrack/ManyTracks では先頭で 1 度だけ読む」分岐を実装する
- VideoFrame / AudioFrame に trackId を持たせる、または Multitrack 専用のラッパー (例: `MultitrackMediaMessage`) を導入するかを実装段階で決定する。既存 API 互換性と no_std 制約を優先する
- SCRIPTDATA で trackId を扱うガイドライン (1644 行以降) は本 issue では対象外とする (Metadata Frame / onMetaData 拡張は 0015 / 0017 が扱う)
- Track Ordering のリコメンデーション (trackId=0 がデフォルト、1, 2, ... が variants) はライブラリの API ドキュメントで言及する

## 完了条件

- VideoPacketType.Multitrack を含む VideoFrame をデコード/エンコードできる (OneTrack / ManyTracks / ManyTracksManyCodecs の各 type で)
- AudioPacketType.Multitrack を含む AudioFrame をデコード/エンコードできる (同上)
- trackId と sizeOfTrack のパース/シリアライズが round-trip する
- 1 つの video / audio message 内に複数トラックを格納したペイロードを正しく往復処理できる
- ユニットテストで以下が確認できる
  - 各 AvMultitrackType の round-trip
  - 1 message 内 N トラックの round-trip (映像/音声それぞれ)
  - 不正な sizeOfTrack でデコード境界を超えた場合のエラー
- pbt の Multitrack 用 Generator が追加され、round-trip プロパティが通る
- fuzz ターゲットが追加されている (Multitrack を含む video/audio message のデコード)
- 既存の単一トラック処理に回帰が無い

## 解決方法

1. `src/media.rs`
   - `AvMultitrackType` enum (OneTrack / ManyTracks / ManyTracksManyCodecs) を定義する
   - VideoFrame / AudioFrame に trackId を表現するフィールドを追加する、または Multitrack 専用構造体を導入する
2. `src/flv.rs`
   - `decode_video_frame` / `decode_audio_frame` で Multitrack 経路を実装し、複数トラックを順次パースする
   - `encode_video_frame` / `encode_audio_frame` で Multitrack 出力を実装する
   - ExVideoTagHeader / ExAudioTagHeader の Multitrack 分岐で `Error::unsupported` を返していた箇所を本実装で置き換える
3. `src/lib.rs`
   - 新規型を re-export する
4. `tests/`
   - Multitrack の round-trip テストを追加する (映像/音声、各 AvMultitrackType)
   - 不正な sizeOfTrack のエラー返却テストを追加する
5. `pbt/`
   - Multitrack 用 Generator を追加する
6. `fuzz/`
   - Multitrack を含む VideoFrame / AudioFrame デコードの fuzz ターゲットを追加する
