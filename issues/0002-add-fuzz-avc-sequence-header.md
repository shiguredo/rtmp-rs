# AvcSequenceHeader::from_bytes() の fuzz ターゲットを追加する

- Priority: Medium
- Created: 2026-05-26
- Model: Opus 4.7
- Branch: feature/add-fuzz-avc-sequence-header

## 目的

`AvcSequenceHeader::from_bytes()` はバイナリパーサーであり、ネストされた長さプレフィクス付きフィールド（SPS/PPS リスト）を手動でオフセット管理しながらパースしている。任意入力に対するパニック安全性を保証するために fuzzing が必要である。

## 優先度根拠

`fuzz_flv` は `decode_video_frame` をカバーするが、`AvcSequenceHeader::from_bytes()` には到達しない。FLV デコーダーは AVC パケットタイプと `composition_timestamp_offset` までは解析するが、`AvcPacketType::SequenceHeader` であっても `data` の内部構造（AVCDecoderConfigurationRecord）のパースは行わない。

## 現状

`src/media.rs` の `AvcSequenceHeader::from_bytes()` に対する fuzz ターゲットが存在しない。単体テストは存在するが、PBT は存在しない（PBT の追加は本 issue のスコープ外とする）。

## 設計方針

- `fuzz/fuzz_targets/fuzz_avc_sequence_header.rs` を新規作成する
- `shiguredo_rtmp::AvcSequenceHeader` を直接インポートする（公開 API として `lib.rs` で `pub use` されている）
- 入力バイト列を `AvcSequenceHeader::from_bytes()` に渡す
- `from_bytes()` が成功した場合は `to_bytes()` も呼び出してパニック安全性を確認する。`from_bytes()` が成功した構造体は (1) SPS/PPS が空でない、(2) 個数が上限以下、(3) 各要素のサイズが `MAX_SPS_SIZE` / `MAX_PPS_SIZE`（4096）以下であり `u16` に収まる。これにより `to_bytes()` の全エラーパスを通過しないため、`unwrap()` で検証する
- ラウンドトリップの等価性検証（`from_bytes -> to_bytes -> from_bytes` で構造体が一致すること）は CLAUDE.md の規約上 PBT の役割であり、本 fuzz ターゲットでは行わない
- `from_bytes()` は末尾の余剰バイトを無視するため、`to_bytes()` の出力と元入力は一般に一致しない
- `fuzz/Cargo.toml` に以下の `[[bin]]` エントリを追加する:

```toml
[[bin]]
name = "fuzz_avc_sequence_header"
path = "fuzz_targets/fuzz_avc_sequence_header.rs"
test = false
doc = false
bench = false
```

## 完了条件

- `fuzz_avc_sequence_header` ターゲットが追加されている
- `cargo fuzz list` に `fuzz_avc_sequence_header` が表示される
- `cargo fuzz run fuzz_avc_sequence_header -- -runs=0` でビルドが通る
- 60 秒間の fuzzing 実行でクラッシュが発生しない
