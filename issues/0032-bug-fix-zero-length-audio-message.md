# 長さ 0 の音声メッセージ（無音）を受理する

- Created: 2026-11-22
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-zero-length-audio-message
- Polished: {YYYY-MM-DD}

## 目的

長さ 0 の音声メッセージを無音（audio silence）として受理し、正規の音声メッセージで接続がエラーにならないようにする。

## 現状

- `refs/enhanced-rtmp-v2.md` の AudioTagHeader の説明には、AudioTagHeader を持たない長さ 0 の音声メッセージを audio silence として扱うという記述がある。
- `src/flv.rs` の `decode_audio_frame` は先頭 1 バイトを必ず読み、空のペイロードでは `ErrorKind::InsufficientBuffer` になる。
- `src/rtmp_message_decoder.rs` の `chunk_to_message` は Audio メッセージで `decode_audio_frame` のエラーをそのまま返すため、接続エラーとして伝播する。
- 同じメッセージ層でも、チャンクデコーダのデータ不足は `Ok(None)` として扱われるが、メッセージ本体のデコードエラーはそのまま返る。

### 再現条件

1. fmt0 のチャンクで、メッセージ型 8（Audio）、message length 0、メッセージストリーム ID 0 を渡す。
2. `RtmpMessageDecoder::decode` を呼ぶ。

期待は無音を表す `AudioFrame`（data 長 0）。実際は `ErrorKind::InsufficientBuffer` のエラーになる。

## 設計方針

- Audio メッセージのペイロードが空の場合、AudioTagHeader を持たない無音メッセージとして扱い、data 長 0 の `AudioFrame` を返す。タイムスタンプはメッセージヘッダーの値を使う。
- 音声フォーマット、サンプルレート、ビット深度、チャンネルは既定値（AAC と同じ固定値など）を使い、値に意味がないことを doc コメントに書く。
- 空のペイロードを無音として扱うのはメッセージ型 8 のみとし、他の型には適用しない。
- 長さ 0 の音声メッセージをエンコードする経路も、必要に応じてサポートするか、明示的にエラーにするかを決める。

## 完了条件

- message length 0 の Audio メッセージがエラーにならず、data 長 0 の `AudioFrame` としてデコードされる。
- 長さ 1 以上の音声メッセージのデコードに回帰がない。
- 無音メッセージのイベントが `AudioReceived` として観測できる。
- 上記を確認する単体テストを追加する。

## 解決方法

1. `src/rtmp_message_decoder.rs` の Audio 分岐で、ペイロードが空の場合に無音 `AudioFrame` を生成する。
2. 必要に応じて `src/flv.rs` の `decode_audio_frame` に空入力を許容する分岐を追加する。
3. 長さ 0 / 1 の境界テストを追加する。
