# 配信サンプルで MP4 の合成時間オフセットを反映する

- Created: 2026-11-23
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-publish-composition-time-offset
- Polished: {YYYY-MM-DD}

## 目的

MP4 の AVC 映像を配信するとき、デコード順で送信しつつ、元の表示時刻をミリ秒の精度で伝えられるようにする。

## 現状

`examples/publish/src/main.rs` の `send_video_sample` は、AVC NALU の `VideoFrame::composition_timestamp_offset` に常に `RtmpTimestampDelta::ZERO` を設定する。`shiguredo_mp4` の `demux::Sample::timestamp` はデコード時刻で、`composition_time_offset` はトラックのタイムスケール単位の表示時刻とデコード時刻の差を表す。

非ゼロのオフセットを持つ AVC サンプルを含む MP4 を配信すると、受信した AVC NALU の `CompositionTime` は 0 になり、元の表示時刻との差が伝わらない。

## 設計方針

トラックのタイムスケールで符号付きオフセットをミリ秒単位に変換し、AVC NALU の `CompositionTime` として送る。ミリ秒未満の扱いを明記し、変換時に符号が変わったり、値が桁落ちやラップアラウンドで別の値になったりしないようにする。ヘッダーの SI24 に収まらない場合はエラーとして扱う。送出の基準となるデコード時刻と AVC シーケンスヘッダーのオフセット 0 は維持する。

## 完了条件

- 正・負のオフセットを持つ AVC サンプルを配信すると、受信側でミリ秒に変換されたオフセットを確認できる。値がない場合は従来どおり 0 になる。
- AVC シーケンスヘッダーのオフセットは 0 のままで、デコード時刻と送信順は変わらない。
- `CompositionTime` に収まらない変換結果は、切り詰めた映像フレームとして送らず、入力エラーになる。
