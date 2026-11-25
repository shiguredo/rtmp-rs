//! RTMP クライアント接続の統合テスト
//!
//! ループバックの TCP 接続を使い、再生クライアントと配信クライアントの
//! 基本的な接続フローを検証する。

/// 再生クライアント接続のテスト
mod play {
    use std::io::{Read, Write};
    use std::net::SocketAddr;
    use std::time::Duration;

    use shiguredo_rtmp::{
        AvcPacketType, RtmpConnectionEvent, RtmpConnectionState, RtmpPlayClientConnection,
        RtmpServerConnection, RtmpTimestamp, RtmpTimestampDelta, RtmpUrl, VideoCodec, VideoFrame,
        VideoFrameType,
    };

    const TEST_STREAM_NAME: &str = "test";

    /// テスト用の RTMP サーバーを起動して待ち受けアドレスを返す
    fn start_rtmp_server_for_play() -> SocketAddr {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("テスト用サーバーの bind に失敗した");
        let addr = listener
            .local_addr()
            .expect("テスト用サーバーのローカルアドレス取得に失敗した");
        println!("テスト用 RTMP サーバーを {addr} で待ち受ける");

        std::thread::spawn(move || run_rtmp_server_for_play(listener));

        addr
    }

    /// 再生クライアント向けの RTMP サーバーを実行する
    fn run_rtmp_server_for_play(listener: std::net::TcpListener) {
        let (mut stream, _) = listener.accept().expect("接続の accept に失敗した");

        let mut conn = RtmpServerConnection::new();
        let mut recv_buf = [0; 4096];

        loop {
            // クライアントからのデータを受信する
            let n = match stream.read(&mut recv_buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock) => 0,
                Err(e) => panic!("データの受信に失敗した: {e}"),
            };

            if n > 0 {
                conn.feed_recv_buf(&recv_buf[..n])
                    .expect("受信バッファの処理に失敗した");
            }

            // 接続から発生したイベントを処理する
            while let Some(event) = conn.next_event() {
                match event {
                    RtmpConnectionEvent::PlayRequested { stream_name, .. } => {
                        println!("ストリーム {stream_name} の再生要求を受信した");
                        assert_eq!(stream_name, TEST_STREAM_NAME, "ストリーム名が一致しない");

                        conn.accept().expect("再生要求の accept に失敗した");

                        // テスト用のダミー映像フレームを送信する
                        let dummy_video_frame = VideoFrame {
                            timestamp: RtmpTimestamp::from_millis(0),
                            composition_timestamp_offset: RtmpTimestampDelta::ZERO,
                            frame_type: VideoFrameType::KeyFrame,
                            codec: VideoCodec::Avc,
                            avc_packet_type: Some(AvcPacketType::NalUnit),
                            data: vec![1, 2, 3, 4],
                        };
                        conn.send_video(dummy_video_frame)
                            .expect("ダミー映像フレームの送信に失敗した");
                    }
                    _ => {
                        // それ以外のイベントは無視する
                    }
                }
            }

            // クライアントへ応答を送信する
            let send_data = conn.send_buf();
            if !send_data.is_empty() {
                (&stream)
                    .write_all(send_data)
                    .expect("クライアントへの書き込みに失敗した");
                conn.advance_send_buf(send_data.len());
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 指定した状態に達するまで接続のハンドシェイクと状態遷移を進める
    ///
    /// ストリームから繰り返し読み込み、接続へデータを渡して応答を書き込む。
    /// 指定した状態に到達するか、最大反復回数を超えるまで続ける。
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
            // ACK などの通知が途中にあっても、キューにある状態遷移を取りこぼさない。
            while let Some(event) = connection.next_event() {
                if event == RtmpConnectionEvent::StateChanged(target_state) {
                    return;
                }
            }

            let n = match stream.read(recv_buf) {
                Ok(0) => panic!("接続が途中で切断された"),
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    0
                }
                Err(e) => panic!("TCP の読み込みに失敗した: {e}"),
            };

            connection
                .feed_recv_buf(&recv_buf[..n])
                .expect("受信バッファの処理に失敗した");

            let send_data = connection.send_buf();
            if !send_data.is_empty() {
                stream
                    .write_all(send_data)
                    .expect("ソケットへの書き込みに失敗した");
                connection.advance_send_buf(send_data.len());
            }

            std::thread::sleep(Duration::from_millis(10));
            iterations += 1;
        }

        if iterations >= max_iterations {
            panic!(
                "{:?} 状態に {} 回の反復以内で到達できなかった",
                target_state, max_iterations
            );
        }
    }

    #[test]
    fn test_rtmp_play_client_basic_flow() {
        // テスト用の RTMP サーバーを起動する
        let server_addr = start_rtmp_server_for_play();

        // サーバーアドレスに接続する
        let mut stream =
            std::net::TcpStream::connect(server_addr).expect("テスト用サーバーへの接続に失敗した");
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("読み込みタイムアウトの設定に失敗した");

        let url_string = format!("rtmp://127.0.0.1/live/{}", TEST_STREAM_NAME);
        let url: RtmpUrl = url_string.parse().expect("URL の解析に失敗した");
        let mut client = RtmpPlayClientConnection::new(url);

        let mut recv_buf = [0; 4096];
        const MAX_ITERATIONS: usize = 1000;

        process_connection_until(
            &mut client,
            &mut stream,
            &mut recv_buf,
            RtmpConnectionState::Playing,
            MAX_ITERATIONS,
        );

        // サーバーから送信されたダミー映像フレームを受信する
        let mut received_video = false;
        for _ in 0..100 {
            let n = match stream.read(&mut recv_buf) {
                Ok(0) => panic!("接続が途中で切断された"),
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    0
                }
                Err(e) => panic!("TCP の読み込みに失敗した: {e}"),
            };

            client
                .feed_recv_buf(&recv_buf[..n])
                .expect("受信バッファの処理に失敗した");

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
                    assert_eq!(expected, frame, "受信した映像フレームが一致しない");
                    received_video = true;
                }
            }

            if received_video {
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(received_video, "映像フレームを受信できなかった");
    }
}

/// 配信クライアント接続のテスト
mod publish {
    use std::io::{Read, Write};
    use std::net::SocketAddr;
    use std::sync::mpsc;
    use std::time::Duration;

    use shiguredo_rtmp::{
        AvcPacketType, MediaFrame, RtmpConnectionEvent, RtmpConnectionState,
        RtmpPublishClientConnection, RtmpServerConnection, RtmpTimestamp, RtmpTimestampDelta,
        RtmpUrl, VideoCodec, VideoFrame, VideoFrameType,
    };

    const TEST_STREAM_NAME: &str = "test";

    /// テスト用の RTMP サーバーを起動し、受信したメディアフレームの受信端を返す
    fn start_rtmp_server() -> (SocketAddr, mpsc::Receiver<MediaFrame>) {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("テスト用サーバーの bind に失敗した");
        let addr = listener
            .local_addr()
            .expect("テスト用サーバーのローカルアドレス取得に失敗した");
        println!("テスト用 RTMP サーバーを {addr} で待ち受ける");

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || run_rtmp_server(listener, tx));

        (addr, rx)
    }

    /// 配信クライアント向けの RTMP サーバーを実行する
    fn run_rtmp_server(listener: std::net::TcpListener, tx: mpsc::Sender<MediaFrame>) {
        let (mut stream, _) = listener.accept().expect("接続の accept に失敗した");

        let mut conn = RtmpServerConnection::new();
        let mut recv_buf = [0; 4096];

        loop {
            // クライアントからのデータを受信する
            let n = stream.read(&mut recv_buf).expect("データの受信に失敗した");
            if n == 0 {
                break;
            }
            conn.feed_recv_buf(&recv_buf[..n])
                .expect("受信バッファの処理に失敗した");

            // 接続から発生したイベントを処理する
            while let Some(event) = conn.next_event() {
                // ガードにすると `frame` をパターンより先にムーブできないため折りたたみはできない
                match event {
                    RtmpConnectionEvent::PublishRequested { stream_name, .. } => {
                        println!("ストリーム {stream_name} の配信要求を受信した");
                        assert_eq!(stream_name, TEST_STREAM_NAME, "ストリーム名が一致しない");

                        conn.accept().expect("配信要求の accept に失敗した");
                    }
                    RtmpConnectionEvent::AudioReceived(frame) => {
                        // 受信側がドロップされている場合はサーバータスクを終了する
                        let media_frame = MediaFrame::Audio(frame);
                        if tx.send(media_frame).is_err() {
                            return;
                        }
                    }
                    RtmpConnectionEvent::VideoReceived(frame) => {
                        // 受信側がドロップされている場合はサーバータスクを終了する
                        let media_frame = MediaFrame::Video(frame);
                        if tx.send(media_frame).is_err() {
                            return;
                        }
                    }
                    _ => {}
                }
            }

            // クライアントへ応答を送信する
            let send_data = conn.send_buf();
            if !send_data.is_empty() {
                (&stream)
                    .write_all(send_data)
                    .expect("クライアントへの書き込みに失敗した");
                conn.advance_send_buf(send_data.len());
            }
        }
    }

    /// 指定した状態に達するまで接続のハンドシェイクと状態遷移を進める
    ///
    /// ストリームから繰り返し読み込み、接続へデータを渡して応答を書き込む。
    /// 指定した状態に到達するか、最大反復回数を超えるまで続ける。
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
            // ACK などの通知が途中にあっても、キューにある状態遷移を取りこぼさない。
            while let Some(event) = connection.next_event() {
                if event == RtmpConnectionEvent::StateChanged(target_state) {
                    return;
                }
            }

            let n = match stream.read(recv_buf) {
                Ok(0) => panic!("接続が途中で切断された"),
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    0
                }
                Err(e) => panic!("TCP の読み込みに失敗した: {e}"),
            };

            connection
                .feed_recv_buf(&recv_buf[..n])
                .expect("受信バッファの処理に失敗した");

            let send_data = connection.send_buf();
            if !send_data.is_empty() {
                stream
                    .write_all(send_data)
                    .expect("ソケットへの書き込みに失敗した");
                connection.advance_send_buf(send_data.len());
            }

            std::thread::sleep(Duration::from_millis(10));
            iterations += 1;
        }

        if iterations >= max_iterations {
            panic!(
                "{:?} 状態に {} 回の反復以内で到達できなかった",
                target_state, max_iterations
            );
        }
    }

    #[test]
    fn test_rtmp_publish_client_basic_flow() {
        // テスト用の RTMP サーバーを起動する
        let (server_addr, media_frame_rx) = start_rtmp_server();

        // サーバーアドレスに接続する
        let mut stream =
            std::net::TcpStream::connect(server_addr).expect("テスト用サーバーへの接続に失敗した");
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("読み込みタイムアウトの設定に失敗した");

        let url_string = format!("rtmp://127.0.0.1/live/{}", TEST_STREAM_NAME);
        let url: RtmpUrl = url_string.parse().expect("URL の解析に失敗した");
        let mut client = RtmpPublishClientConnection::new(url);

        let mut recv_buf = [0; 4096];
        const MAX_ITERATIONS: usize = 1000;

        process_connection_until(
            &mut client,
            &mut stream,
            &mut recv_buf,
            RtmpConnectionState::Publishing,
            MAX_ITERATIONS,
        );

        // ダミーの空バイト列を映像フレームとして送信する
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
            .expect("映像フレームの送信に失敗した");

        let send_data = client.send_buf();
        if !send_data.is_empty() {
            (&stream)
                .write_all(send_data)
                .expect("ソケットへの書き込みに失敗した");
            client.advance_send_buf(send_data.len());
        }

        // サーバーにメディアフレームが届いたことを確認する
        let MediaFrame::Video(frame) = media_frame_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("メディアフレームの受信に失敗した")
        else {
            panic!("受信したメディアフレームが映像ではない");
        };
        assert_eq!(dummy_frame, frame, "受信した映像フレームが一致しない");
    }
}
