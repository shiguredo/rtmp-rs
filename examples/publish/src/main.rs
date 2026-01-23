//! RTMP/RTMPS パブリッシュクライアントの例 (tokio + tokio-rustls)
//!
//! 使い方:
//!   # RTMP (従来通り)
//!   cargo run -p publish -- -H 127.0.0.1 -p 1935 -a live -s test input.mp4
//!
//!   # RTMPS (--tls フラグ)
//!   cargo run -p publish -- -H example.com -p 443 -a live -s test --tls input.mp4
//!
//!   # RTMPS (URL スキーム)
//!   cargo run -p publish -- -u rtmps://example.com/live/test input.mp4

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use rustls_platform_verifier::ConfigVerifierExt;
use shiguredo_mp4::demux::{Input, Mp4FileDemuxer};
use shiguredo_rtmp::{
    AudioFormat, AudioFrame, AvcPacketType, RtmpPublishClientConnection, RtmpTimestamp,
    RtmpTimestampDelta, RtmpUrl, VideoCodec, VideoFrame, VideoFrameType,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[tokio::main]
async fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = "publish";
    args.metadata_mut().app_description = "Publish MP4 file to RTMP/RTMPS server";

    noargs::HELP_FLAG.take_help(&mut args);

    // URL オプション (rtmp:// または rtmps:// スキーム)
    let url: Option<RtmpUrl> = noargs::opt("url")
        .short('u')
        .doc("RTMP/RTMPS URL (e.g., rtmps://example.com/live/stream)")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;

    let host: String = noargs::opt("host")
        .short('H')
        .doc("RTMP server host")
        .default("127.0.0.1")
        .take(&mut args)
        .then(|o| o.value().parse())?;
    let port: Option<u16> = noargs::opt("port")
        .short('p')
        .doc("RTMP server port (default: 1935, or 443 with --tls)")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;
    let app: String = noargs::opt("app")
        .short('a')
        .doc("RTMP application name")
        .default("live")
        .take(&mut args)
        .then(|o| o.value().parse())?;
    let stream_name: String = noargs::opt("stream")
        .short('s')
        .doc("RTMP stream name")
        .default("stream")
        .take(&mut args)
        .then(|o| o.value().parse())?;
    let tls_flag: bool = noargs::flag("tls")
        .doc("Enable TLS (RTMPS)")
        .take(&mut args)
        .is_present();
    let verbose: bool = noargs::flag("verbose")
        .doc("Enable verbose output")
        .take(&mut args)
        .is_present();
    let input_file_path: PathBuf = noargs::arg("<INPUT_MP4_FILE>")
        .doc("Input MP4 file path")
        .example("/path/to/input.mp4")
        .take(&mut args)
        .then(|a| a.value().parse())?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    // URL が指定されている場合はそれを使用、そうでなければ個別のオプションから構築
    let rtmp_url = if let Some(url) = url {
        url
    } else {
        let use_tls = tls_flag;
        let final_port = port.unwrap_or(if use_tls { 443 } else { 1935 });
        RtmpUrl {
            host,
            port: final_port,
            app,
            stream_name,
            tls: use_tls,
        }
    };

    println!("Publishing to RTMP:");
    println!("  RTMP URL:   {rtmp_url}");
    println!("  Input file: {}", input_file_path.display());
    if rtmp_url.tls {
        println!("  TLS:        enabled");
    }

    publish_mp4_to_rtmp(&rtmp_url, &input_file_path, verbose).await?;

    Ok(())
}

/// Plain/TLS を抽象化した RTMP ストリーム
enum RtmpStream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl RtmpStream {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            RtmpStream::Plain(s) => s.read(buf).await,
            RtmpStream::Tls(s) => s.read(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            RtmpStream::Plain(s) => s.write_all(buf).await,
            RtmpStream::Tls(s) => s.write_all(buf).await,
        }
    }
}

async fn publish_mp4_to_rtmp(
    url: &RtmpUrl,
    input_file_path: &PathBuf,
    verbose: bool,
) -> noargs::Result<()> {
    // MP4 ファイルを読み込む
    // （サンプルコードでは簡単のために一度に全てのデータを読み込んでしまっている）
    let file_data = std::fs::read(input_file_path)?;

    // MP4 ファイルをデマルチプレックスして初期化
    let mut demuxer = Mp4FileDemuxer::new();
    let input = Input {
        position: 0,
        data: &file_data,
    };
    demuxer.handle_input(input);

    // トラック情報を取得＆コーデック検証
    let tracks = demuxer.tracks()?;
    println!("Found {} track(s)", tracks.len());

    validate_codecs(&mut demuxer)?;

    // 再度デマルチプレックスを初期化してから配信を開始
    let mut demuxer = Mp4FileDemuxer::new();
    demuxer.handle_input(input);

    // RTMP クライアント接続を作成
    let mut connection = RtmpPublishClientConnection::new(url.clone());

    // ソケットに接続
    let mut socket = if url.tls {
        connect_tls(&url.host, url.port).await?
    } else {
        connect_plain(&url.host, url.port).await?
    };

    println!("Connected to RTMP server, starting to publish...");

    run_publishing_loop(
        &mut connection,
        &mut socket,
        &mut demuxer,
        &file_data,
        verbose,
    )
    .await?;

    Ok(())
}

async fn connect_plain(host: &str, port: u16) -> noargs::Result<RtmpStream> {
    let stream = TcpStream::connect(format!("{host}:{port}")).await?;
    Ok(RtmpStream::Plain(stream))
}

async fn connect_tls(host: &str, port: u16) -> noargs::Result<RtmpStream> {
    // TLS 設定 (システムのプラットフォーム証明書ストアを使用)
    let config = ClientConfig::with_platform_verifier()
        .map_err(|e| format!("Failed to create TLS config: {e}"))?;

    let connector = TlsConnector::from(Arc::new(config));

    let server_name =
        ServerName::try_from(host.to_string()).map_err(|e| format!("Invalid server name: {e}"))?;

    let stream = TcpStream::connect(format!("{host}:{port}")).await?;
    let tls_stream = connector
        .connect(server_name, stream)
        .await
        .map_err(|e| format!("TLS handshake failed: {e}"))?;

    Ok(RtmpStream::Tls(Box::new(tls_stream)))
}

async fn run_publishing_loop(
    connection: &mut RtmpPublishClientConnection,
    socket: &mut RtmpStream,
    demuxer: &mut Mp4FileDemuxer,
    file_data: &[u8],
    verbose: bool,
) -> noargs::Result<()> {
    let mut recv_buf = vec![0u8; 8192];
    let start_time = Instant::now();
    let mut sample_count = 0;
    let mut publishing = false;
    let mut nalu_length_size: u8 = 4; // 典型的な値をデフォルト値にしておく
    let mut last_mp4a_box: Option<shiguredo_mp4::boxes::Mp4aBox> = None;

    // イベント処理ループ
    loop {
        // イベント処理
        while let Some(event) = connection.next_event() {
            // イベントハンドリング
            if verbose {
                dbg!(&event);
            }
            if let shiguredo_rtmp::RtmpConnectionEvent::StateChanged(s) = event
                && s == shiguredo_rtmp::RtmpConnectionState::Publishing
            {
                publishing = true;
            }
        }

        // 送信バッファをソケットに書き込む
        let send_data = connection.send_buf();
        if !send_data.is_empty() {
            socket.write_all(send_data).await.ok();
            connection.advance_send_buf(send_data.len());
        }

        // ソケットからデータを受信 (タイムアウト付き)
        match tokio::time::timeout(Duration::from_millis(5), socket.read(&mut recv_buf)).await {
            Ok(Ok(0)) => break, // 接続が切断された
            Ok(Ok(n)) => connection.feed_recv_buf(&recv_buf[..n])?,
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
            Ok(Err(e)) => Err(e)?,
            Err(_) => {} // タイムアウト（一度配信が始まったら、ほとんどの場合はここに来る）
        }

        if !publishing {
            continue;
        }

        // サンプルを送信
        let Some(sample) = demuxer.next_sample()? else {
            break; // 全てのサンプルを送信した
        };

        // タイムスタンプに基づいて配信ペースを制御する
        let elapsed = start_time.elapsed();
        let target_time = Duration::from_secs(sample.timestamp) / sample.track.timescale.get();
        tokio::time::sleep(target_time.saturating_sub(elapsed)).await;

        let timestamp_ms = target_time.as_millis() as u32;
        let sample_data =
            &file_data[sample.data_offset as usize..sample.data_offset as usize + sample.data_size];

        // トラックの種類に応じてフレームを送信
        match sample.track.kind {
            shiguredo_mp4::TrackKind::Video => {
                send_video_sample(
                    connection,
                    &sample,
                    sample_data,
                    timestamp_ms,
                    &mut nalu_length_size,
                )?;
                sample_count += 1;
            }
            shiguredo_mp4::TrackKind::Audio => {
                send_audio_sample(
                    connection,
                    &sample,
                    sample_data,
                    timestamp_ms,
                    &mut last_mp4a_box,
                )?;
                sample_count += 1;
            }
        }
    }

    println!("Publishing completed ({sample_count} samples sent)");

    Ok(())
}

/// 映像サンプルを処理してサーバーに送信する
fn send_video_sample(
    connection: &mut RtmpPublishClientConnection,
    sample: &shiguredo_mp4::demux::Sample,
    sample_data: &[u8],
    timestamp_ms: u32,
    nalu_length_size: &mut u8,
) -> noargs::Result<()> {
    // ビデオトラックの SPS/PPS を取得
    let mut video_sps_list = Vec::new();
    let mut video_pps_list = Vec::new();
    if let Some(sample_entry) = sample.sample_entry
        && let shiguredo_mp4::boxes::SampleEntry::Avc1(avc1_box) = sample_entry
    {
        video_sps_list = avc1_box.avcc_box.sps_list.clone();
        video_pps_list = avc1_box.avcc_box.pps_list.clone();
        *nalu_length_size = avc1_box.avcc_box.length_size_minus_one.get() + 1;
    }

    // キーフレームの場合は SequenceHeader を先に送信する
    if sample.keyframe && !video_sps_list.is_empty() {
        let seq_header_data = create_avc_sequence_header_annexb(&video_sps_list, &video_pps_list);
        let seq_frame = VideoFrame {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            composition_timestamp_offset: RtmpTimestampDelta::ZERO,
            frame_type: VideoFrameType::KeyFrame,
            codec: VideoCodec::Avc,
            avc_packet_type: Some(AvcPacketType::SequenceHeader),
            data: seq_header_data,
        };
        connection.send_video(seq_frame)?;
        println!("Sent AVC Sequence Header");
    }

    // 映像データ本体を送信する
    let annexb_data = convert_nalu_to_annexb(sample_data, *nalu_length_size);
    let frame = VideoFrame {
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        composition_timestamp_offset: RtmpTimestampDelta::ZERO, // B フレームは存在しない前提
        frame_type: if sample.keyframe {
            VideoFrameType::KeyFrame
        } else {
            VideoFrameType::InterFrame
        },
        codec: VideoCodec::Avc,
        avc_packet_type: Some(AvcPacketType::NalUnit),
        data: annexb_data,
    };
    connection.send_video(frame)?;
    Ok(())
}

/// 音声サンプルを処理してサーバーに送信する
fn send_audio_sample(
    connection: &mut RtmpPublishClientConnection,
    sample: &shiguredo_mp4::demux::Sample,
    sample_data: &[u8],
    timestamp_ms: u32,
    last_mp4a_box: &mut Option<shiguredo_mp4::boxes::Mp4aBox>,
) -> noargs::Result<()> {
    let is_first = last_mp4a_box.is_none();

    // サンプルエントリーから Mp4aBox を取得または再利用
    if let Some(entry) = sample.sample_entry
        && let shiguredo_mp4::boxes::SampleEntry::Mp4a(mp4a) = entry
    {
        // 新しいサンプルエントリーが来た場合は更新
        *last_mp4a_box = Some(mp4a.clone());
    }

    let mp4a_box = last_mp4a_box
        .as_ref()
        .ok_or("No audio sample entry available")?;
    let is_8bit = mp4a_box.audio.samplesize == 8;

    // シーケンスヘッダーを送信する（最初のサンプルの場合）
    if is_first && let Some(audio_config) = create_aac_audio_specific_config(mp4a_box) {
        let seq_frame = AudioFrame {
            timestamp: RtmpTimestamp::from_millis(timestamp_ms),
            format: AudioFormat::Aac,
            sample_rate: AudioFrame::AAC_SAMPLE_RATE,
            is_stereo: AudioFrame::AAC_STEREO,
            is_8bit_sample: is_8bit,
            is_aac_sequence_header: true,
            data: audio_config,
        };
        connection.send_audio(seq_frame)?;
        println!("Sent AAC Sequence Header");
    }

    // 音声データ本体を送信する
    let frame = AudioFrame {
        timestamp: RtmpTimestamp::from_millis(timestamp_ms),
        format: AudioFormat::Aac,
        sample_rate: AudioFrame::AAC_SAMPLE_RATE,
        is_stereo: AudioFrame::AAC_STEREO,
        is_8bit_sample: is_8bit,
        is_aac_sequence_header: false,
        data: sample_data.to_vec(),
    };
    connection.send_audio(frame)?;
    Ok(())
}

/// AAC の AudioSpecificConfig を作成する
fn create_aac_audio_specific_config(mp4a_box: &shiguredo_mp4::boxes::Mp4aBox) -> Option<Vec<u8>> {
    // EsdsBox から DecoderSpecificInfo を取得
    mp4a_box.esds_box.es.dec_config_descr.dec_specific_info.as_ref().map(|dec_specific_info| dec_specific_info.payload.clone())
}

/// MP4 ファイルの H.264 映像フレームの形式を RTMP がサポートしている Annex B 形式に変換する
fn convert_nalu_to_annexb(data: &[u8], length_size: u8) -> Vec<u8> {
    let mut result = Vec::new();
    let mut offset = 0;
    let length_size = length_size as usize;

    while offset < data.len() {
        if offset + length_size > data.len() {
            break;
        }

        // MP4 ファイル形式で H.264 の NALU 長を読み取る
        let length = match length_size {
            1 => data[offset] as usize,
            2 => u16::from_be_bytes([data[offset], data[offset + 1]]) as usize,
            3 => u32::from_be_bytes([0, data[offset], data[offset + 1], data[offset + 2]]) as usize,
            4 => u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize,
            _ => {
                unreachable!() // MP4 ライブラリがチェックしているのでここには来ないはず
            }
        };

        offset += length_size;

        if offset + length > data.len() {
            break;
        }

        // Annex B の形式（先頭に固定の区切りバイト列が付与される）に変換する
        result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        result.extend_from_slice(&data[offset..offset + length]);

        offset += length;
    }

    result
}

/// H.264 のシーケンスヘッダ を RTMP がサポートしている Annex B 形式で作成する
fn create_avc_sequence_header_annexb(sps_list: &[Vec<u8>], pps_list: &[Vec<u8>]) -> Vec<u8> {
    let mut result = Vec::new();
    for sps in sps_list {
        result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        result.extend_from_slice(sps);
    }
    for pps in pps_list {
        result.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        result.extend_from_slice(pps);
    }
    result
}

// 入力ファイルのコーデックが H.264 / AAC かどうかをチェックする
fn validate_codecs(demuxer: &mut Mp4FileDemuxer) -> noargs::Result<()> {
    let mut has_h264_video = false;
    let mut has_aac_audio = false;

    while let Some(sample) = demuxer.next_sample()? {
        if let Some(sample_entry) = sample.sample_entry {
            match sample_entry {
                shiguredo_mp4::boxes::SampleEntry::Avc1(_) => {
                    has_h264_video = true;
                    println!("✓ Found H.264 video codec");
                }
                shiguredo_mp4::boxes::SampleEntry::Mp4a(_) => {
                    has_aac_audio = true;
                    println!("✓ Found AAC audio codec");
                }
                other => {
                    // サポートされていないコーデックは警告として記録
                    println!("⚠ Unsupported codec found: {other:?}");
                }
            }
        }
    }

    // ビデオまたはオーディオが見つからない場合は警告
    if !has_h264_video {
        println!("⚠ No H.264 video codec found");
    }
    if !has_aac_audio {
        println!("⚠ No AAC audio codec found");
    }

    // 映像も音声も利用可能なものが見つからなかった場合はエラー
    if !has_h264_video && !has_aac_audio {
        return Err(
            "No supported codecs found. At least H.264 video or AAC audio is required.".into(),
        );
    }

    Ok(())
}
