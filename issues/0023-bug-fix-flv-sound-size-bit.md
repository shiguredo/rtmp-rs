# FLV 音声タグの SoundSize（8 bit / 16 bit）の解釈を修正する

- Created: 2026-11-22
- Completed: 2026-11-25
- Branch: feature/change-flv-sound-size
- Polished: 2026-11-25

## 目的

legacy FLV の音声タグで SoundSize の極性を仕様に合わせ、非圧縮音声の 8 ビットと 16 ビットを正しく識別できるようにする。

## 現状

- [Adobe FLV 仕様 10.1 §E.4.2.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=76) の AudioTagHeader では、非圧縮音声の SoundSize を 0 = 8 ビット、1 = 16 ビットと定義している。
- `decode_audio_frame` は `(header & 0x02) != 0` を `is_8bit_sample` としている。`encode_audio_frame` も `is_8bit_sample` をそのままビット 1 に書き込む。両方向で誤りが相殺されるため、往復テストだけでは検出できない。

### 再現条件

44 kHz・モノラルのリトルエンディアン PCM（SoundFormat 3）として、`decode_audio_frame(&[0x3E, 0x34, 0x12], RtmpTimestamp::ZERO)` を呼ぶ。SoundSize は 1 なので期待値は `is_8bit_sample = false` だが、現行実装は `true` を返す。同じ音声データを `is_8bit_sample = false` のフレームからエンコードしても、現行実装はヘッダーを `0x3C` にする。

## 設計方針

- Speex を除く対応済み legacy 形式では、`encode_audio_frame` と `decode_audio_frame` が SoundSize 0 と `is_8bit_sample = true`、SoundSize 1 と `false` を対応付ける。圧縮音声では実サンプル幅ではなくヘッダーの値を表す。PCM 形式 0 の追加は後続の 0041 で行う。
- [Adobe FLV 仕様 10.1 §E.4.2.1](https://veovera.github.io/enhanced-rtmp/docs/legacy/video-file-format-v10-1-spec.pdf#page=77) は Speex の SoundSize を 1 と規定する。送信では常に 1 とし、受信は入力ビットを使わず 1 として扱う。仕様は受信側の拒否を要求していない。SoundRate、SoundType、AACPacketType、ペイロードの扱いは今回変えない。
- `AudioFrame::is_8bit_sample` の名前と型は維持し、ヘッダーとの対応と Speex の例外を公開文書に明記する。同じ入力で返すフラグと同じフレームから出力するビットが変わるため、変更履歴に `[CHANGE]` を記載する。旧版でビットが逆に出力された PCM は値だけでは識別できないため、自動補正しない。

## 完了条件

- 44 kHz・モノラル・リトルエンディアン PCM の 8 ビットは `[0x3C, 0x80]`、16 ビットは `[0x3E, 0x34, 0x12]` とする。デコードではそれぞれ `is_8bit_sample = true` と `false` を返し、ペイロードを保持する。各フレームをエンコードしても同じバイト列になることを、デコード結果を使わずにそれぞれ検証する。
- Speex は仕様に従い SoundRate のビット値 0、SoundType のビット値 0（モノラル）で検証する。`0xB0` と `0xB2` のヘッダーをどちらも受理し、フラグは `false` にする。同じフィールドを指定して書き込むと、どちらのフラグでも `0xB2` になり、ペイロードは変わらない。
- 公開 API の性質テストで固定バイト列との照合に加え、既存の FLV と RTMP 音声メッセージの往復を確認する。FLV の往復では Speex の送信時の正規化を考慮し、他の形式ではフレームとペイロードが一致することを確認する。

## 解決方法

- `encode_audio_frame` と `decode_audio_frame` の共通の極性を `true` と 0、`false` と 1 に修正した。Speex は送信時に必ず 1 を書き、受信時は入力ビットを使わない。ペイロードと他のヘッダーフィールドは変更しない。RTMP の音声メッセージも同じ処理を利用する。
- PCM の 8 ビット・16 ビット、Speex の固定ヘッダーを、往復に依存せず公開 API の読み書きで照合した。FLV と RTMP の往復性質テストが通り、修正前後の比較でも受信後の再送が変わるのは Speex の規格外のビットを 1 にする場合だけだった。
- 公開フィールドと変更履歴に極性と Speex の扱いを記載した。旧版の極性に合わせた補正は不要になるが、既存の PCM データが旧版から出力されたかはヘッダーだけでは判別できない。
