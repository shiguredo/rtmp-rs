use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::Duration;

use shiguredo_rtmp::{
    AvcPacketType, RtmpConnectionEvent, RtmpConnectionState, RtmpPlayClientConnection,
    RtmpServerConnection, RtmpTimestamp, RtmpTimestampDelta, VideoCodec, VideoFrame,
    VideoFrameType,
};

const TEST_STREAM_NAME: &str = "test";

fn start_rtmp_server_for_play() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind server");
    let addr = listener.local_addr().expect("Failed to get local address");
    println!("Test RTMP server listening on {addr}");

    std::thread::spawn(move || run_rtmp_server_for_play(listener));

    addr
}

fn run_rtmp_server_for_play(listener: std::net::TcpListener) {
    let (mut stream, _) = listener.accept().expect("Failed to accept connection");

    let mut conn = RtmpServerConnection::new();
    let mut recv_buf = [0; 4096];

    loop {
        // Read incoming data
        let n = match stream.read(&mut recv_buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock) => 0,
            Err(e) => panic!("Read error: {e}"),
        };

        if n > 0 {
            conn.feed_recv_buf(&recv_buf[..n])
                .expect("Failed to feed receive buffer");
        }

        // Process events
        while let Some(event) = conn.next_event() {
            dbg!(&event);
            match event {
                RtmpConnectionEvent::PlayRequested { stream_name, .. } => {
                    println!("Play requested for stream: {stream_name}");
                    assert_eq!(stream_name, TEST_STREAM_NAME);

                    conn.accept().expect("Failed to accept play");

                    // Send a dummy video frame
                    let dummy_video_frame = VideoFrame {
                        timestamp: RtmpTimestamp::from_millis(0),
                        composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                        frame_type: VideoFrameType::KeyFrame,
                        codec: VideoCodec::Avc,
                        avc_packet_type: Some(AvcPacketType::NalUnit),
                        data: vec![1, 2, 3, 4],
                    };
                    conn.send_video(dummy_video_frame)
                        .expect("Failed to send dummy video frame");
                }
                _ => {
                    // Ignore other events
                }
            }
        }

        // Send response
        let send_data = conn.send_buf();
        if !send_data.is_empty() {
            dbg!(send_data.len());
            (&stream)
                .write_all(send_data)
                .expect("Failed to write to stream");
            conn.advance_send_buf(send_data.len());
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Helper function to process RTMP connection handshake/state transitions
///
/// Repeatedly reads from the stream, feeds data to the connection, and writes responses
/// until the specified target state is reached or max iterations are exceeded.
#[track_caller]
fn process_connection_until(
    connection: &mut RtmpPlayClientConnection,
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
fn test_rtmp_play_client_basic_flow() {
    // Start test RTMP server
    let server_addr = start_rtmp_server_for_play();

    // Connect to server address
    let mut stream =
        std::net::TcpStream::connect(server_addr).expect("Failed to connect to test server");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("Failed to set read timeout");

    let mut client = RtmpPlayClientConnection::new("rtmp://127.0.0.1/live", TEST_STREAM_NAME);

    let mut recv_buf = [0; 4096];
    const MAX_ITERATIONS: usize = 1000;

    process_connection_until(
        &mut client,
        &mut stream,
        &mut recv_buf,
        RtmpConnectionState::Playing,
        MAX_ITERATIONS,
    );

    // Receive the dummy video frame sent by the server
    let mut received_video = false;
    for i in 0..100 {
        dbg!(i);
        let n = match stream.read(&mut recv_buf) {
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

        client.feed_recv_buf(&recv_buf[..n]).expect("Feed failed");

        while let Some(event) = client.next_event() {
            if let RtmpConnectionEvent::VideoReceived(frame) = event {
                let expected = VideoFrame {
                    timestamp: RtmpTimestamp::from_millis(0),
                    composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                    frame_type: VideoFrameType::KeyFrame,
                    codec: VideoCodec::Avc,
                    avc_packet_type: Some(AvcPacketType::NalUnit),
                    data: vec![1, 2, 3, 4],
                };
                assert_eq!(expected, frame);
                received_video = true;
            }
        }

        if received_video {
            break;
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(received_video, "Failed to receive video frame");
}
