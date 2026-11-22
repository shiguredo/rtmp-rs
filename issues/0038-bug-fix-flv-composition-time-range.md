# FLV 映像タグの CompositionTime の範囲検証を追加する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-flv-composition-time-range
- Polished: {YYYY-MM-DD}

## 目的

CompositionTime（SI24）に収まらない値をエンコード時に拒否し、タイムスタンプが無言で変わるのを防ぐ。

## 現状

- `src/flv.rs` の `encode_video_frame` は `composition_timestamp_offset.as_millis()` を `write_i24` で書き込む。`write_i24` は下位 24 ビットだけを書き込む。
- `src/bytes.rs` の `BytesWriter::write_i24` に範囲検証はない。
- CompositionTime は FLV 仕様の VideoTagHeader で SI24（-2^23〜2^23-1）と定義されている。

### 再現条件

1. `VideoFrame` を `codec = VideoCodec::Avc`、`avc_packet_type = Some(AvcPacketType::NalUnit)`、`composition_timestamp_offset = RtmpTimestampDelta::from_millis(0x0100_0000)` で構築する。
2. エンコードしてデコードし、`composition_timestamp_offset` を比較する。

期待はエラー、または元の値の保持。実際は値が切り詰められて異なる値になる。

## 設計方針

- エンコード時に `-2^23..=2^23-1` の範囲を検証し、範囲外は `ErrorKind::InvalidData` にする。`encode_video_frame` の戻り値を `Result<(), Error>` にする必要がある。
- デコード側は SI24 の符号拡張を維持する。
- `RtmpTimestampDelta` の doc と実際に表現できる範囲の関係を確認し、必要なら doc を修正する。
- 範囲内の値の往復に回帰がないことを確認する。

## 完了条件

- `-2^23` と `2^23-1` はエンコード・デコードで往復する。
- `2^23` と `-2^23-1` はエンコード時にエラーになる。
- 既存の AVC フレームの往復に回帰がない。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `encode_video_frame` で CompositionTime の範囲を検証する。
2. 戻り値を `Result` に変更し、呼び出し側を修正する。
3. 境界値のテストを追加する。
