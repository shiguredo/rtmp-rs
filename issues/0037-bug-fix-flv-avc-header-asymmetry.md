# FLV 映像タグの AVC ヘッダー条件のエンコードとデコードの非対称を修正する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-flv-avc-header-asymmetry
- Polished: {YYYY-MM-DD}

## 目的

`VideoFrame` の公開フィールドの組み合わせによって、エンコードとデコードで異なるフレームになる問題を解消する。

## 現状

- `src/flv.rs` の `encode_video_frame` は `frame.avc_packet_type` が `Some` なら、コーデックが AVC かどうかに関係なく AVC パケットタイプと CompositionTime の 4 バイトを書き込む。
- `src/flv.rs` の `decode_video_frame` はコーデックが `VideoCodec::Avc` で、フレームタイプが `VideoFrameType::VideoInfoOrCommandFrame` でない場合にだけ AVC ヘッダーを読む。
- エンコード側の条件とデコード側の条件が一致していない。

### 再現条件

1. `VideoFrame` を `codec = VideoCodec::Vp6`、`frame_type = VideoFrameType::KeyFrame`、`avc_packet_type = Some(AvcPacketType::NalUnit)`、`data = [0xAA, 0xBB]` で構築する。
2. `encode_video_frame` でエンコードし、`decode_video_frame` でデコードする。

期待は元のフレームと同じ値。実際は `avc_packet_type` が `None` になり、`data` の先頭に AVC ヘッダーの 4 バイト `[0x01, 0x00, 0x00, 0x00]` が混入する。

## 設計方針

- エンコード時に「AVC パケットタイプは AVC コーデックでのみ有効」という不変条件を検証し、違反時はエラーを返す。`encode_video_frame` を `Result<(), Error>` に変更する。
- あるいはデコード側の条件に合わせて、非 AVC コーデックでは `avc_packet_type` を無視して AVC ヘッダーを書かないようにする。どちらを採用するかは公開 API の破壊的変更の許容度で決める。
- `VideoInfoOrCommandFrame` の扱いも、エンコードとデコードで同じ条件にする。
- 既存の AVC フレームの往復に回帰がないことを確認する。

## 完了条件

- 非 AVC コーデックに `avc_packet_type = Some` を指定した場合に、エラーになるか、AVC ヘッダーを書かずに往復するかのどちらかに統一される。
- AVC コーデックの `SequenceHeader`、`NalUnit`、`EndOfSequence` の往復に回帰がない。
- `VideoInfoOrCommandFrame` の扱いがエンコードとデコードで一致する。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `encode_video_frame` でコーデックと `avc_packet_type` の整合性を検証する。
2. 必要に応じて戻り値を `Result` に変更し、呼び出し側を修正する。
3. 非 AVC + `Some`、AVC + `None`、`VideoInfoOrCommandFrame` のケースをテストに追加する。
