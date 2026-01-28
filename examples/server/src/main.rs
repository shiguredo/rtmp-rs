//! RTMP/RTMPS サーバーの例 (tokio + tokio-rustls)
//!
//! 使い方:
//!   # RTMP サーバー (ポート 1935)
//!   cargo run -p server
//!
//!   # RTMPS サーバー (ポート 443)
//!   cargo run -p server -- --tls --cert cert.pem --key key.pem
//!
//! テスト用の自己署名証明書の作成:
//!   openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes \
//!     -subj "/CN=localhost"

use std::collections::HashMap;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use shiguredo_rtmp::{
    AudioFrame, MediaFrame, RtmpConnectionEvent, RtmpServerConnection, VideoFrame,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc};
use tokio_rustls::TlsAcceptor;

struct ServerOptions {
    host: String,
    port: u16,
    tls: bool,
    cert_path: Option<String>,
    key_path: Option<String>,
    verbose: bool,
}

#[tokio::main]
async fn main() -> noargs::Result<()> {
    let options = parse_args()?;

    let addr = format!("{}:{}", options.host, options.port);
    let listener = TcpListener::bind(&addr).await?;

    let media_streams = Arc::new(Mutex::new(HashMap::new()));
    let mut client_id: usize = 0;

    if options.tls {
        let cert_path = options
            .cert_path
            .as_ref()
            .ok_or("--cert is required for TLS")?;
        let key_path = options
            .key_path
            .as_ref()
            .ok_or("--key is required for TLS")?;

        let config = load_tls_config(cert_path, key_path)?;
        let acceptor = TlsAcceptor::from(Arc::new(config));

        println!("RTMPS server listening on rtmps://{}", addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let acceptor = acceptor.clone();
            let media_streams = media_streams.clone();
            let verbose = options.verbose;

            client_id += 1;
            let id = client_id;

            tokio::spawn(async move {
                match acceptor.accept(stream).await {
                    Ok(tls_stream) => {
                        println!("New TLS connection from: {peer_addr}");
                        let handler = ClientConnectionHandler::new_tls(
                            id,
                            tls_stream,
                            media_streams,
                            verbose,
                        );
                        if let Err(e) = handler.run().await {
                            println!("[ERROR] {e}");
                        }
                        println!("TLS disconnected: {peer_addr}");
                    }
                    Err(e) => eprintln!("TLS handshake error from {}: {}", peer_addr, e),
                }
            });
        }
    } else {
        println!("RTMP server listening on rtmp://{}", addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let media_streams = media_streams.clone();
            let verbose = options.verbose;

            client_id += 1;
            let id = client_id;

            tokio::spawn(async move {
                println!("New TCP connection from: {peer_addr}");
                let handler =
                    ClientConnectionHandler::new_plain(id, stream, media_streams, verbose);
                if let Err(e) = handler.run().await {
                    println!("[ERROR] {e}");
                }
                println!("TCP disconnected: {peer_addr}");
            });
        }
    }
}

fn parse_args() -> noargs::Result<ServerOptions> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = "server";
    args.metadata_mut().app_description = "RTMP/RTMPS server";

    noargs::HELP_FLAG.take_help(&mut args);

    let tls: bool = noargs::flag("tls")
        .doc("Enable RTMPS (TLS)")
        .take(&mut args)
        .is_present();

    let default_port = if tls { "443" } else { "1935" };

    let host: String = noargs::opt("host")
        .short('H')
        .doc("Listen host address")
        .default("0.0.0.0")
        .take(&mut args)
        .then(|o| o.value().parse())?;
    let port: u16 = noargs::opt("port")
        .short('p')
        .doc("Listen port number (default: 1935, or 443 with --tls)")
        .default(default_port)
        .take(&mut args)
        .then(|o| o.value().parse())?;
    let verbose: bool = noargs::flag("verbose")
        .doc("Enable verbose output")
        .take(&mut args)
        .is_present();
    let cert_path: Option<String> = noargs::opt("cert")
        .doc("Path to certificate file (PEM)")
        .take(&mut args)
        .present_and_then(|o| Ok::<_, &str>(o.value().to_string()))?;
    let key_path: Option<String> = noargs::opt("key")
        .doc("Path to private key file (PEM)")
        .take(&mut args)
        .present_and_then(|o| Ok::<_, &str>(o.value().to_string()))?;

    if let Some(help) = args.finish()? {
        print!("{help}");
        std::process::exit(0);
    }

    Ok(ServerOptions {
        host,
        port,
        tls,
        cert_path,
        key_path,
        verbose,
    })
}

fn load_tls_config(cert_path: &str, key_path: &str) -> noargs::Result<ServerConfig> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert_path)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read certificates: {e}"))?;

    if certs.is_empty() {
        Err("No certificates found in cert file")?;
    }

    let key = PrivateKeyDer::from_pem_file(key_path)
        .map_err(|e| format!("Failed to read private key: {e}"))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("Failed to create TLS config: {e}"))?;

    Ok(config)
}

type StreamId = String; // "{app}/{stream_name}"

enum RtmpStream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::server::TlsStream<TcpStream>>),
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

struct ClientConnectionHandler {
    shared_state: Arc<Mutex<HashMap<StreamId, MediaStreamState>>>,
    client_id: usize,
    stream: RtmpStream,
    conn: RtmpServerConnection,
    recv_buf: [u8; 4096],
    media_tx: mpsc::Sender<MediaFrame>,
    media_rx: mpsc::Receiver<MediaFrame>,
    stream_id: StreamId,
    received_keyframe: bool,
    verbose: bool,
}

impl ClientConnectionHandler {
    fn new_plain(
        client_id: usize,
        stream: TcpStream,
        shared_state: Arc<Mutex<HashMap<StreamId, MediaStreamState>>>,
        verbose: bool,
    ) -> Self {
        Self::new(client_id, RtmpStream::Plain(stream), shared_state, verbose)
    }

    fn new_tls(
        client_id: usize,
        stream: tokio_rustls::server::TlsStream<TcpStream>,
        shared_state: Arc<Mutex<HashMap<StreamId, MediaStreamState>>>,
        verbose: bool,
    ) -> Self {
        Self::new(
            client_id,
            RtmpStream::Tls(Box::new(stream)),
            shared_state,
            verbose,
        )
    }

    fn new(
        client_id: usize,
        stream: RtmpStream,
        shared_state: Arc<Mutex<HashMap<StreamId, MediaStreamState>>>,
        verbose: bool,
    ) -> Self {
        let (media_tx, media_rx) = mpsc::channel(1024);
        Self {
            client_id,
            stream,
            shared_state,
            conn: RtmpServerConnection::new(),
            recv_buf: [0; 4096],
            media_tx,
            media_rx,
            stream_id: String::new(),
            received_keyframe: false,
            verbose,
        }
    }

    async fn run(mut self) -> Result<(), Error> {
        let mut result = Ok(());
        loop {
            match self.run_one().await {
                Err(e) => {
                    result = Err(e);
                    break; // 異常終了
                }
                Ok(false) => break, // 正常終了
                Ok(true) => {}      // 処理継続
            }
        }
        self.cleanup().await;
        result
    }

    async fn run_one(&mut self) -> Result<bool, Error> {
        // イベント処理
        while let Some(event) = self.conn.next_event() {
            self.process_event(event).await?;
        }

        // 送信バッファにデータがあれば送信
        while !self.conn.send_buf().is_empty() {
            let send_data = self.conn.send_buf();
            self.stream.write_all(send_data).await?;
            let len = send_data.len();
            self.conn.advance_send_buf(len);
        }

        tokio::select! {
            // 配信側から送られてきたフレームを受信側に転送する
            Some(frame) = self.media_rx.recv() => {
                self.process_media_frame(frame)?;
            }

            // ソケットからデータを受信
            result = self.stream.read(&mut self.recv_buf) => {
                match result {
                    Ok(0) => return Ok(false), // 接続が切断された
                    Ok(n) => {
                        self.conn.feed_recv_buf(&self.recv_buf[..n])?;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => return Ok(false),
                    Err(e) => return Err(e.into()),
                }
            }
        }
        Ok(true)
    }

    fn process_media_frame(&mut self, frame: MediaFrame) -> Result<(), Error> {
        match frame {
            MediaFrame::Audio(frame) => self.conn.send_audio(frame)?,
            MediaFrame::Video(frame) => {
                if !self.received_keyframe {
                    if !frame.is_keyframe() {
                        // 映像はキーフレームが届くまではドロップする
                        return Ok(());
                    }
                    self.received_keyframe = true;
                }
                self.conn.send_video(frame)?;
            }
        }
        Ok(())
    }

    async fn process_event(&mut self, event: RtmpConnectionEvent) -> Result<(), Error> {
        if self.verbose
            && !matches!(
                event,
                RtmpConnectionEvent::AudioReceived(_) | RtmpConnectionEvent::VideoReceived(_)
            )
        {
            dbg!(&event);
        }

        match event {
            RtmpConnectionEvent::PublishRequested {
                app, stream_name, ..
            } => {
                let stream_id = format!("{app}/{stream_name}");
                self.handle_publish_requested(stream_id).await?;
            }
            RtmpConnectionEvent::PlayRequested {
                app, stream_name, ..
            } => {
                let stream_id = format!("{app}/{stream_name}");
                self.handle_play_requested(stream_id).await?;
            }
            RtmpConnectionEvent::AudioReceived(frame) => {
                self.handle_audio_received(frame).await?;
            }
            RtmpConnectionEvent::VideoReceived(frame) => {
                self.handle_video_received(frame).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_publish_requested(&mut self, stream_id: StreamId) -> Result<(), Error> {
        let mut streams = self.shared_state.lock().await;

        // 他の配信者がすでに存在するかどうかをチェックする
        if let Some(state) = streams.get(&stream_id)
            && state.publisher_id.is_some()
        {
            let reason =
                format!("Stream '{stream_id}' is already being published by another client");
            self.conn.reject(&reason)?;
            return Err(Error::new(reason));
        }

        // 同じストリーム ID に対する配信者が存在しないなら許可する
        self.conn.accept()?;

        let state = streams.entry(stream_id.clone()).or_default();
        state.publisher_id = Some(self.client_id);
        self.stream_id = stream_id;
        Ok(())
    }

    async fn handle_play_requested(&mut self, stream_id: StreamId) -> Result<(), Error> {
        // Play 要求は無条件で許可する
        self.conn.accept()?;

        let mut streams = self.shared_state.lock().await;
        streams
            .entry(stream_id.clone())
            .or_default()
            .players
            .insert(
                self.client_id,
                PlayerState {
                    is_sequence_header_sent: false,
                    is_keyframe_sent: false,
                    tx: self.media_tx.clone(),
                },
            );
        self.stream_id = stream_id;
        Ok(())
    }

    async fn handle_audio_received(&mut self, frame: AudioFrame) -> Result<(), Error> {
        let streams = self.shared_state.lock().await;
        if let Some(state) = streams.get(&self.stream_id) {
            for player in state.players.values() {
                let _ = player.tx.send(MediaFrame::Audio(frame.clone())).await;
            }
        }
        Ok(())
    }

    async fn handle_video_received(&mut self, frame: VideoFrame) -> Result<(), Error> {
        let mut streams = self.shared_state.lock().await;
        if let Some(state) = streams.get_mut(&self.stream_id) {
            if frame.is_keyframe()
                && frame.avc_packet_type == Some(shiguredo_rtmp::AvcPacketType::SequenceHeader)
            {
                state.sequence_header_frame = Some(frame.clone());
            }
            for player in state.players.values_mut() {
                if !player.is_keyframe_sent && !frame.is_keyframe() {
                    player.is_keyframe_sent = true;
                    continue;
                }
                if !player.is_sequence_header_sent {
                    if let Some(f) = state.sequence_header_frame.clone() {
                        let _ = player.tx.send(MediaFrame::Video(f)).await;
                        player.is_sequence_header_sent = true;
                    } else {
                        continue;
                    }
                }
                let _ = player.tx.send(MediaFrame::Video(frame.clone())).await;
            }
        }
        Ok(())
    }

    async fn cleanup(&mut self) {
        if let Ok(mut streams) = self.shared_state.try_lock()
            && let Some(state) = streams.get_mut(&self.stream_id)
        {
            // 共有状態からクライアントの情報を削除する
            if state.publisher_id == Some(self.client_id) {
                state.publisher_id = None;
                state.sequence_header_frame = None;
            }

            state.players.remove(&self.client_id);
        }

        // 送信バッファに残っているデータをベストエフォートで送信する
        let _ = self.stream.write_all(self.conn.send_buf()).await;
    }
}

#[derive(Debug, Default)]
struct MediaStreamState {
    publisher_id: Option<usize>,
    sequence_header_frame: Option<VideoFrame>,
    players: HashMap<usize, PlayerState>,
}

struct PlayerState {
    is_sequence_header_sent: bool,
    is_keyframe_sent: bool,
    tx: mpsc::Sender<MediaFrame>,
}

impl std::fmt::Debug for PlayerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerState")
            .field("is_sequence_header_sent", &self.is_sequence_header_sent)
            .field("is_keyframe_sent", &self.is_keyframe_sent)
            .finish()
    }
}

#[derive(Debug)]
struct Error {
    reason: String,
    backtrace: std::backtrace::Backtrace,
}

impl Error {
    fn new(reason: String) -> Self {
        Self {
            reason,
            backtrace: std::backtrace::Backtrace::capture(),
        }
    }
}

impl<T: std::error::Error> From<T> for Error {
    fn from(t: T) -> Self {
        Self::new(t.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)?;
        if self.backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            write!(f, "\n{}", self.backtrace)?;
        }
        Ok(())
    }
}
