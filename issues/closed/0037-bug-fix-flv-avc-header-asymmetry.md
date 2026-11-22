# FLV 映像タグの AVC ヘッダー条件のエンコードとデコードの非対称を修正する

- Created: 2026-11-22
- Completed: 2026-11-23
- Branch: feature/fix-flv-avc-header-asymmetry
- Polished: 2026-11-22

## 目的

legacy FLV の映像タグで、AVC 追加ヘッダーの有無をエンコードとデコードで一致させる。エンコード時にコーデックと `avc_packet_type` の矛盾を拒否し、AVC 追加ヘッダーが映像データやコマンドに混入しないようにする。

## 現状

- `src/flv.rs` の `encode_video_frame` は `frame.avc_packet_type` が `Some` なら、コーデックが AVC かどうかに関係なく AVC パケットタイプと CompositionTime の 4 バイトを書き込む。
- `src/flv.rs` の `decode_video_frame` はコーデックが `VideoCodec::Avc` で、フレームタイプが `VideoFrameType::VideoInfoOrCommandFrame` でない場合にだけ AVC ヘッダーを読む。
- エンコード側の条件とデコード側の条件が一致していない。

### 再現条件

1. `VideoFrame` を `timestamp = RtmpTimestamp::ZERO`、`composition_timestamp_offset = RtmpTimestampDelta::ZERO`、`codec = VideoCodec::Vp6`、`frame_type = VideoFrameType::KeyFrame`、`avc_packet_type = Some(AvcPacketType::NalUnit)`、`data = [0xAA, 0xBB]` で構築する。
2. `encode_video_frame` に渡す。現行実装で生成したバイト列を同じ `timestamp` で `decode_video_frame` に渡す。

期待値はエンコード時の `ErrorKind::InvalidInput`。現行実装では `[0x14, 0x01, 0x00, 0x00, 0x00, 0xAA, 0xBB]` が出力される。デコード結果は `avc_packet_type = None` になり、`data` に AVC 追加ヘッダーの 4 バイト `[0x01, 0x00, 0x00, 0x00]` が混入する。

## 設計方針

- [Adobe FLV 仕様 10.1 §E.4.3.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=78) の VideoTagHeader は、`CodecID == 7` の場合に `AVCPacketType`（UI8）と `CompositionTime`（SI24）を置く。エンコードとデコードはともに `codec == VideoCodec::Avc` の場合だけこの 4 バイトを扱う。`FrameType == 5` の UI8 は VideoTagBody の規定であり、AVC 追加ヘッダーを省略する条件ではない。`VideoInfoOrCommandFrame` のコマンドバイトはヘッダーに混ぜず、`data` の一部としてそのまま扱う。
- `VideoCodec::Avc` には `avc_packet_type = Some`、その他のコーデックには `None` を要求する。組み合わせが不正なら `encode_video_frame` はバッファへの書き込み前に `ErrorKind::InvalidInput` を返す。パケットタイプを推測・無視して続行しない。公開 `VideoFrame` のフィールド構成は変えない。
- 公開 `encode_video_frame` を `Result<(), Error>` にし、`RtmpMessageEncoder::encode` と `RtmpMessageChannel::feed_send_message` も `Result<(), Error>` でエラーを伝える。配信クライアントとサーバーの `send_video` は既存の `Result` で失敗を返す。失敗したメッセージは送信バッファに加えず、後続メッセージのチャンクエンコード状態も変えない。送信経路が先に `Result` 化されていれば再利用する。
- 今回の検証は AVC 追加ヘッダーの有無に限定する。`CompositionTime` の SI24 値域は 0038 で検証し、`Result` の伝播は本 issue の実装を利用する。パケットタイプ別の時刻の値、非 AVC でワイヤ上に表現されないオフセット、映像・コマンドのボディの検証は変更しない。Enhanced RTMP の新形式は 0018 の対象とする。

## 完了条件

- 非 AVC + `Some` と AVC + `None` は `ErrorKind::InvalidInput` となり、あらかじめデータを入れた出力バッファを変更しない。RTMP の送信経路も同じエラーを返し、既存の送信バッファと後続のチャンクエンコードに影響しない。
- AVC の通常フレームは、既存の `SequenceHeader`、`NalUnit`、`EndOfSequence` のワイヤ表現と往復を維持する。正しい非 AVC + `None` には AVC 追加ヘッダーを書かない。
- AVC の `VideoInfoOrCommandFrame` では FrameType / CodecID の 1 バイト、AVC 追加ヘッダーの 4 バイト、コマンドの順で処理する。例として `[0x57, 0x01, 0x00, 0x00, 0x00, 0x01]` は `NalUnit`、CompositionTime 0、`data = [0x01]` として読み書きできる。AVC で追加ヘッダーを省きコマンド 1 バイトだけを続けた `[0x57, 0x01]` は、ヘッダーが足りないためデコードに失敗する。非 AVC のコマンドには AVC 追加ヘッダーがない。
- 正常系は既存の FLV と RTMP メッセージの往復 PBT を更新して検証する。コマンドの上記バイト列は、往復の一致だけでなくエンコードとデコードを別々に照合する。不正な組み合わせとエラー時の出力不変も公開 API で回帰を確認する。単体テストを用いるのは PBT で実現できないケースに限る。

## 解決方法

- `encode_video_frame` と `decode_video_frame` の両方で、フレームタイプを問わず AVC の場合だけ追加ヘッダーを扱う。コマンドはヘッダーの後ろにそのまま置く。コーデックと AVC パケットタイプの有無が矛盾する場合は、書き込み前に `InvalidInput` を返す。公開型の文書ではヘッダーとコマンドのボディを区別した。
- `encode_video_frame`、`RtmpMessageEncoder::encode`、`RtmpMessageChannel::feed_send_message` は `Result<(), Error>` を返す。ペイロード生成に失敗した場合はチャンクの出力・状態更新をせず、配信クライアントとサーバーの `send_video` も同じエラーを返す。
- FLV と RTMP の往復用の生成条件を更新し、符号化・復号のワイヤ配置、短い AVC ヘッダーの拒否、誤った指定での送信バッファと後続チャンクの不変を公開 API のテストで確認した。fuzz ターゲットも戻り値の変更に対応した。
