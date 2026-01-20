use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::mpsc;
use std::time::Duration;

use shiguredo_rtmp::{
    AvcPacketType, MediaFrame, RtmpTimestamp, RtmpTimestampDelta, VideoCodec, VideoFrame,
    VideoFrameType,
};
use shiguredo_rtmp::{
    RtmpConnectionEvent, RtmpConnectionState, RtmpPublishClientConnection, RtmpServerConnection,
};

const TEST_STREAM_NAME: &str = "test";

fn start_rtmp_server() -> (SocketAddr, mpsc::Receiver<MediaFrame>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind server");
    let addr = listener.local_addr().expect("Failed to get local address");
    println!("Test RTMP server listening on {addr}");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || run_rtmp_server(listener, tx));

    (addr, rx)
}

fn run_rtmp_server(listener: std::net::TcpListener, tx: mpsc::Sender<MediaFrame>) {
    let (mut stream, _) = listener.accept().expect("Failed to accept connection");

    let mut conn = RtmpServerConnection::new();
    let mut recv_buf = [0; 4096];

    loop {
        // Read incoming data
        let n = stream.read(&mut recv_buf).expect("Failed to read");
        if n == 0 {
            break;
        }
        conn.feed_recv_buf(&recv_buf[..n])
            .expect("Failed to feed receive buffer");

        // Process events
        while let Some(event) = conn.next_event() {
            dbg!(&event);
            match event {
                RtmpConnectionEvent::PublishRequested { stream_name, .. } => {
                    println!("Publish requested for stream: {stream_name}");
                    assert_eq!(stream_name, TEST_STREAM_NAME);

                    conn.accept().expect("Failed to accept publish");
                }
                RtmpConnectionEvent::AudioReceived(frame) => {
                    if tx.send(MediaFrame::Audio(frame)).is_err() {
                        return;
                    }
                }
                RtmpConnectionEvent::VideoReceived(frame) => {
                    if tx.send(MediaFrame::Video(frame)).is_err() {
                        return;
                    }
                }
                _ => {}
            }
        }

        // Send response
        let send_data = conn.send_buf();
        if !send_data.is_empty() {
            (&stream)
                .write_all(send_data)
                .expect("Failed to write to stream");
            conn.advance_send_buf(send_data.len());
        }
    }
}

/// Helper function to process RTMP connection handshake/state transitions
///
/// Repeatedly reads from the stream, feeds data to the connection, and writes responses
/// until the specified target state is reached or max iterations are exceeded.
#[track_caller]
fn process_connection_until(
    connection: &mut RtmpPublishClientConnection,
    stream: &mut std::net::TcpStream,
    recv_buf: &mut [u8],
    target_state: RtmpConnectionState,
    max_iterations: usize,
) {
    let mut iterations = 0;
    while iterations < max_iterations {
        while let Some(RtmpConnectionEvent::StateChanged(state)) = connection.next_event() {
            if state == target_state {
                return;
            }
        }

        let n = match stream.read(recv_buf) {
            Ok(0) => panic!("Unexpected EOF"),
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                0
            }
            Err(e) => panic!("TCP read error: {e}"),
        };

        connection
            .feed_recv_buf(&recv_buf[..n])
            .expect("Feed failed");

        let send_data = connection.send_buf();
        if !send_data.is_empty() {
            dbg!("write", send_data.len());
            stream.write_all(send_data).expect("Failed to socket write");
            connection.advance_send_buf(send_data.len());
        }

        std::thread::sleep(Duration::from_millis(10));
        iterations += 1;
    }

    if iterations >= max_iterations {
        panic!(
            "Failed to reach state {:?} within {} iterations",
            target_state, max_iterations
        );
    }
}

#[test]
fn test_rtmp_publish_client_basic_flow() {
    // テスト用の RTMP サーバーを起動
    let (server_addr, media_frame_rx) = start_rtmp_server();

    // サーバーアドレスに接続
    let mut stream =
        std::net::TcpStream::connect(server_addr).expect("Failed to connect to test server");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("Failed to set read timeout");

    let mut client = RtmpPublishClientConnection::new("rtmp://127.0.0.1/live", TEST_STREAM_NAME);

    let mut recv_buf = [0; 4096];
    const MAX_ITERATIONS: usize = 1000;

    process_connection_until(
        &mut client,
        &mut stream,
        &mut recv_buf,
        RtmpConnectionState::Publishing,
        MAX_ITERATIONS,
    );

    // ダミーの空バイト列をビデオフレームとして送信
    let dummy_frame = VideoFrame {
        timestamp: RtmpTimestamp::from_millis(0),
        composition_timestamp_offset: RtmpTimestampDelta::ZERO,
        frame_type: VideoFrameType::KeyFrame,
        codec: VideoCodec::Avc,
        avc_packet_type: Some(AvcPacketType::NalUnit),
        data: vec![], // 空バイト列
    };
    client
        .send_video(dummy_frame.clone())
        .expect("Failed to send video");

    let send_data = client.send_buf();
    if !send_data.is_empty() {
        (&stream).write_all(send_data).expect("Write failed");
        client.advance_send_buf(send_data.len());
    }

    // サーバーに届いたことを確認する
    let MediaFrame::Video(frame) = media_frame_rx
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("failed to recv media frame")
    else {
        panic!("not video frame");
    };
    assert_eq!(dummy_frame, frame);
}
