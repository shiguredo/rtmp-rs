# Enhanced RTMP の Multitrack (映像/音声) 対応を追加する

- Priority: High
- Created: 2026-06-26
- Completed: {YYYY-MM-DD}
- Model: Opus 4.7
- Branch: feature/add-enhanced-rtmp-multitrack
- Polished: 2026-07-28

## 目的

Enhanced RTMP v2 の Multitrack Streaming 仕様 (映像/音声) に対応し、1 つの video / audio message 内に複数トラック (異なるビットレート/解像度/コーデック/言語/カメラアングル) を多重化して送受信できるようにする。

## 優先度根拠

High。eRTMP の主要な拡張機能の 1 つであり、Sora の SFU 配信ユースケース (1 接続で複数視点/品質) と直結する。
仕様の `CapsExMask.Multitrack` フラグでクライアント能力として明示的に宣言される機能であり、eRTMP 完全準拠を掲げる以上、対応必須。

## 現状

- ExVideoTagHeader 基盤 (0018) と ExAudioTagHeader 基盤 (0009) は未完了であり、`VideoPacketType` / `AudioPacketType` 型はコードベースに存在しない。本 issue は 0018 と 0009 の完了が前提となる
- 0018 / 0009 完了時点では、`VideoPacketType.Multitrack` / `AudioPacketType.Multitrack` を受信すると `ErrorKind::Unsupported` が返る。本 issue は `decode_video_message` / `decode_audio_message` 内で Multitrack 経路を実装する。既存の `decode_video_frame` / `decode_audio_frame` の Multitrack 分岐は `Error::unsupported` のまま維持する (後方互換)
- 既存の VideoFrame / AudioFrame 構造体は単一トラック前提であり、trackId / sizeOfTrack の概念が存在しない

## 設計方針

- Enhanced RTMP v2 の Multitrack Streaming セクション (`refs/enhanced-rtmp-v2.md` の 1611 行以降) および ExVideoTagHeader / ExAudioTagHeader の Multitrack 分岐に準拠する
- 0018 / 0009 で整備される ExVideoTagHeader / ExAudioTagHeader 基盤の上に、Multitrack 経路を実装する
- ワイヤフォーマット (仕様の該当行):
  - ExVideoTagHeader の Multitrack 分岐 (仕様 1216-1238 行): `videoPacketType == VideoPacketType.Multitrack` の場合、`isVideoMultitrack = true`、`videoMultitrackType = UB[4] as AvMultitrackType`、続いて全トラック共通の `videoPacketType = UB[4] as VideoPacketType` を読む。このネストした PacketType は Multitrack であってはならない (仕様 1221 行 MUST)。`ManyTracksManyCodecs` 以外の場合は先頭で 1 度だけ `videoFourCc = FOURCC` を読む
  - ExAudioTagHeader の Multitrack 分岐 (仕様 823-837 行): 映像と同一構造。`audioPacketType == AudioPacketType.Multitrack` の場合、`isAudioMultitrack = true`、`audioMultitrackType = UB[4] as AvMultitrackType`、続いて `audioPacketType = UB[4] as AudioPacketType` を読む (Multitrack 不可、仕様 828 行 MUST)。`ManyTracksManyCodecs` 以外の場合は先頭で 1 度だけ `audioFourCc = FOURCC` を読む
  - ExVideoTagBody の Multitrack 処理 (仕様 1249-1304 行): `while (processVideoBody)` ループでトラックを順次パースする。`ManyTracksManyCodecs` の場合は各トラック直前で `videoFourCc = FOURCC` を読み直す。各トラックは `videoTrackId = UI8` + (`OneTrack` 以外の場合は `sizeOfVideoTrack = UI24`) + ペイロードで構成される。`sizeOfVideoTrack` はこの値の直後からカウントを開始し、次のトラックの先頭を指すオフセットである (仕様 1295-1301 行)
  - ExAudioTagBody の Multitrack 処理 (仕様 897-952 行): 映像と同一構造。`audioTrackId = UI8` + (`OneTrack` 以外の場合は `sizeOfAudioTrack = UI24`) + ペイロード
- enum 定義 (数値はワイヤフォーマットそのもの。仕様 1231-1238 行 / 879-886 行):
  - `AvMultitrackType`: OneTrack=0, ManyTracks=1, ManyTracksManyCodecs=2 (映像/音声共通)
- エラー分類:
  - ネストした PacketType が Multitrack を返した場合: `Error::invalid_data` (仕様 MUST 違反)
  - AvMultitrackType 予約値 (3-15): `Error::invalid_data`
  - sizeOfTrack が残りバイト数を超えた場合: `Error::invalid_data`
  - 0018 / 0009 のエラー分類を継承する
- データモデル:
  - VideoFrame / AudioFrame に `track_id: Option<u8>` フィールドを追加する (非 Multitrack フレームは None、Multitrack 内の各トラックは Some(trackId))。これは公開 struct へのフィールド追加であり破壊的変更を伴う (0018 / 0009 で既にデータモデル再構成が行われている前提の上での追加変更)
  - Multitrack メッセージのデコード結果は `Vec<VideoFrame>` / `Vec<AudioFrame>` として返す。既存の `decode_video_frame` / `decode_audio_frame` は単一フレームを返すシグネチャを維持し、Multitrack を受信した場合は `Error::unsupported` を返し続ける (後方互換)。新たに `decode_video_message` / `decode_audio_message` を追加し、Multitrack の場合は複数フレームの Vec を、非 Multitrack の場合は単一要素の Vec を返す。0009 の `DecodedAudio` 設計が確定した場合は、`decode_audio_message` の返り値型をそれに合わせて調整する (0009 は設計を二択で未確定としているため、0014 実装時に 0009 の確定結果を参照する)
  - エンコードは `encode_video_message(buf, &[VideoFrame]) -> Result<(), Error>` / `encode_audio_message(buf, &[AudioFrame]) -> Result<(), Error>` を追加する。`track_id=None` の単一フレームの場合は既存の `encode_video_frame` / `encode_audio_frame` と同一出力。`track_id=Some` を含む場合は Multitrack 形式で書き出す。AvMultitrackType の選択規則: 全フレームのコーデックが同一なら ManyTracks、異なるコーデックが混在すれば ManyTracksManyCodecs、単一フレームかつ track_id=Some なら OneTrack。入力検証: track_id の None/Some 混在は `Error::invalid_data`、空スライスは `Error::invalid_data`、track_id=None の複数フレームは `Error::invalid_data`、フレーム間 timestamp 不一致は `Error::invalid_data`
  - ネストした AudioPacketType が MultichannelConfig の場合: 0009 で実装済みの MultichannelConfig パースを再利用し、`Vec<AudioFrame>` の先頭要素に MultichannelConfig 情報を付与する (0009 のデータモデル確定後に具体的な格納先を決定する)
- round-trip の定義: pbt の round-trip プロパティは model-level (decode(encode(x)) == x) で検証する。wire-level (encode(decode(y)) == y) は AvMultitrackType の正規化 (ManyTracksManyCodecs で全トラック同一コーデック → ManyTracks に化ける等) により構造的に成立しないため、wire-level のプロパティは要求しない
- Track Ordering のリコメンデーション (仕様 1257-1291 行 / 903-939 行: trackId=0 がデフォルトトラック、1, 2, ... が variants) は `AvMultitrackType` の rustdoc で言及する
- SCRIPTDATA で trackId を扱うガイドライン (仕様 1644 行以降) は本 issue では対象外とする (Metadata Frame / onMetaData 拡張は 0015 / 0017 が扱う)
- 単一 video / audio message に複数の AudioPacketType / VideoPacketType が入り得るという Important 注記 (仕様 1092 行 / 741 行) は、主に Multitrack のバッチを想定したものである。本 issue でこのうち Multitrack のトラックループを実装する (Metadata バッチは 0015 の担当)
- スコープ境界: 本 issue は flv 層 (src/flv.rs) の Multitrack プリミティブ追加のみを扱う。rtmp_message 層 (src/rtmp_message_decoder.rs / src/rtmp_message_encoder.rs) での Multitrack メッセージ統合 (RtmpMessage::Audio/Video が複数フレームを保持する設計) は後続 issue で対応する。本 issue 完了時点では flv 層のデコード/エンコード関数が利用可能になるが、RTMP 接続層での end-to-end な Multitrack 送受信は後続 issue の完了を待つ

## 完了条件

- VideoPacketType.Multitrack を含む video message をデコード/エンコードできる (OneTrack / ManyTracks / ManyTracksManyCodecs の各 type で)
- AudioPacketType.Multitrack を含む audio message をデコード/エンコードできる (同上)
- ネストした PacketType が Multitrack の場合に `Error::invalid_data` になる
- AvMultitrackType 予約値 (3-15) が `Error::invalid_data` になる
- sizeOfTrack が残りバイト数を超えた場合に `Error::invalid_data` になる
- `encode_video_message` / `encode_audio_message` の入力検証 (空スライス、track_id の None/Some 混在、track_id=None の複数フレーム、timestamp 不一致) が `Error::invalid_data` になる
- trackId と sizeOfTrack のパース/シリアライズが round-trip する
- 1 つの video / audio message 内に複数トラックを格納したペイロードを正しく往復処理できる
- 既存の `decode_video_frame` / `decode_audio_frame` は Multitrack 受信時に `Error::unsupported` を返し続ける (後方互換)
- ユニットテストで上記の各ケースが確認できる
- pbt: Multitrack 用 Generator が追加され、model-level round-trip プロパティ (decode(encode(x)) == x) が通る
- fuzz: 既存 `fuzz/fuzz_targets/fuzz_flv.rs` が `decode_video_frame` / `decode_audio_frame` をカバーしているため、新規ターゲットは `decode_video_message` / `decode_audio_message` に対して追加する (`fuzz/fuzz_targets/fuzz_multitrack.rs`)
- 既存の単一トラック処理に回帰が無い (既存テストがすべて通る)
- CHANGES.md に CHANGE (VideoFrame / AudioFrame への track_id フィールド追加) + ADD (新規関数群) として記載されている

## 解決方法

1. `src/media.rs`
   - `AvMultitrackType` enum (OneTrack=0 / ManyTracks=1 / ManyTracksManyCodecs=2) を定義する
   - VideoFrame / AudioFrame に `track_id: Option<u8>` フィールドを追加する
2. `src/flv.rs`
   - `decode_video_message` / `decode_audio_message` を追加し、Multitrack 経路 (トラックループ、sizeOfTrack によるオフセット計算、ManyTracksManyCodecs の FourCC 読み直し) を実装する
   - `encode_video_message` / `encode_audio_message` を追加し、Multitrack 出力を実装する
   - `decode_video_frame` / `decode_audio_frame` の Multitrack 分岐は `Error::unsupported` のまま維持する (後方互換)
3. `src/lib.rs`
   - 新規型 (`AvMultitrackType`) と新規関数 (`decode_video_message` / `decode_audio_message` / `encode_video_message` / `encode_audio_message`) を re-export する
4. `tests/` / `pbt/` / `fuzz/` / `examples/`
   - 完了条件に列挙したテスト・Generator・fuzz ターゲットを追加する
   - `track_id` フィールド追加に伴うコンパイル修正 (VideoFrame / AudioFrame 構築箇所) を行う
5. `CHANGES.md`
   - CHANGE (track_id フィールド追加) + ADD (新規関数群) として記載する (`shiguredo-changelog` スキル参照)
