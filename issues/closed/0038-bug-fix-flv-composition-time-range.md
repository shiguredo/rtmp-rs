# FLV 映像タグの CompositionTime の範囲検証を追加する

- Created: 2026-11-22
- Completed: 2026-11-23
- Branch: feature/fix-flv-composition-time-range
- Polished: 2026-11-22

## 目的

legacy FLV の AVC 追加ヘッダーに書く `CompositionTime` が SI24 の表現範囲に収まらない場合は、エンコード時に拒否し、合成時間のオフセットが切り詰められるのを防ぐ。

## 現状

- `src/flv.rs` の `encode_video_frame` は、`avc_packet_type` が `Some` のとき `composition_timestamp_offset.as_millis()` を `write_i24` に渡す。`src/bytes.rs` の `BytesWriter::write_i24` は範囲を検証せず、下位 24 ビットのみを書き込む。
- `RtmpTimestampDelta` は `i32` を保持する。[Adobe FLV 仕様 10.1 §E.4.3.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=78) の VideoTagHeader では、AVC の `CompositionTime` はミリ秒単位の SI24 で、表現範囲は `-8_388_608..=8_388_607`（`-2^23` から `2^23 - 1`）に限られる。

### 再現条件

1. `VideoFrame` を `timestamp = RtmpTimestamp::ZERO`、`frame_type = VideoFrameType::InterFrame`、`codec = VideoCodec::Avc`、`avc_packet_type = Some(AvcPacketType::NalUnit)`、`composition_timestamp_offset = RtmpTimestampDelta::from_millis(0x0100_0000)`、`data = []` で構築する。
2. `encode_video_frame` に渡す。現行実装で生成したバイト列を同じ `timestamp` で `decode_video_frame` に渡し、オフセットを確認する。

期待値はエンコード時の `ErrorKind::InvalidInput`。現行実装では `0x0100_0000` の下位 24 ビットだけが `[0x00, 0x00, 0x00]` として書き込まれ、デコード結果のオフセットは 0 ミリ秒になる。

## 設計方針

- 0037 を先に実装し、そこで決めたヘッダー条件とエラー伝播を再利用する。`encode_video_frame` で legacy AVC の追加ヘッダーを出力する場合だけ、`CompositionTime` が `-8_388_608..=8_388_607` に収まるか検証する。範囲外なら最初のヘッダーバイトを書き込む前に `ErrorKind::InvalidInput` を返し、0037 の `Result` 経路で RTMP の呼び出し元へ伝える。非 AVC では `CompositionTime` を書かず、値域検証もしない。
- Adobe 仕様では `AVCPacketType` が `NalUnit` でない場合の `CompositionTime` は 0 だが、本 issue では値域だけを検証し、範囲内の値を 0 に強制しない。`BytesWriter::write_i24` とデコード時の SI24 の符号拡張は変更しない。
- `RtmpTimestampDelta` の保持型とコンストラクターの `i32` は変更しない。同型の doc コメントでは符号付き 32 ビットを型の格納範囲として説明し、[Adobe RTMP 仕様 §4](https://veovera.org/docs/legacy/rtmp-v1-0-spec.pdf#page=5) にある符号なしのチャンクのタイムスタンプ差分とは区別する。`VideoFrame::composition_timestamp_offset` と `encode_video_frame` の doc コメントには、legacy FLV の AVC ヘッダーへ書き出す場合は SI24 の値域に収める必要があることを記載する。

## 完了条件

- AVC の `NalUnit` で `-8_388_608` と `8_388_607` はそれぞれ `[0x80, 0x00, 0x00]` と `[0x7F, 0xFF, 0xFF]` として書き込まれ、デコード後も同じ値になる。
- `-8_388_609`、`8_388_608`、および再現例の `0x0100_0000` は AVC のエンコード時に `ErrorKind::InvalidInput` となる。直接の出力バッファ、RTMP の送信バッファ、後続のチャンクエンコード状態には変更を残さない。
- 既存の FLV と RTMP メッセージの往復 PBT に上下限を含め、範囲内の回帰がないことを確認する。範囲外の拒否と出力不変も公開 API で検証する。単体テストの追加は PBT で実現できないケースに限る。

## 解決方法

- `encode_video_frame` は、AVC 追加ヘッダーを書き込むときだけ、先頭バイトの出力前に `CompositionTime` が SI24 の範囲に収まるか検査する。範囲外なら `InvalidInput` を返し、範囲内の値は補正しない。非 AVC の扱いとデコードは変更していない。
- 既存の RTMP 送信経路で同じエラーを伝える。公開文書には SI24 の制約と、`RtmpTimestampDelta` の格納範囲およびチャンク時刻差分との違いを記載した。
- 既存の FLV と RTMP の往復テストに上下限を含め、FLV では固定バイト列も照合した。範囲外の値で直接の出力と送信待ちのバイト列を変えず、後続チャンクにも影響しないことを検証した。
