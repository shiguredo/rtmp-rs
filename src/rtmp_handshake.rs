//! RTMP のハンドシェイク（C0/C1/C2 と S0/S1/S2）を処理するモジュール
//! ハンドシェイクの手順と各パケットの形式は RTMP 仕様 5.2 節に従う。

use alloc::vec::Vec;
use core::mem;

use crate::bytes::{BytesReader, BytesWriter};
use crate::error::Error;

// [補足]
// Flash Player が廃止された今となっては、
// ハンドシェイク時のパラメーターを細かく制御する意義はないと思われるので、
// 問題が発生するまでは固定値を使用しておく
const RTMP_VERSION: u8 = 3;
const HANDSHAKE_PACKET_SIZE: usize = 1536;
// Adobe RTMP 仕様（2012 年 12 月版）5.2.2～5.2.4 節の片方向のバイト数。
// 別形式のハンドシェイクを扱うようになった場合は計上方法を見直す。
pub(crate) const HANDSHAKE_BYTES: u64 = 1 + 2 * HANDSHAKE_PACKET_SIZE as u64;
const APP_VERSION: [u8; 4] = [0, 0, 0, 0]; // 一番小さな値を使うことで、途中から導入されたダイジェスト形式のサポートを不要にしている
const RANDOM_DATA: [u8; HANDSHAKE_PACKET_SIZE - 8] = [0; HANDSHAKE_PACKET_SIZE - 8]; // 固定値で別に困らないので固定値にしておく
const TIMESTAMP: u32 = 0; // 同上

#[derive(Debug, Clone)]
struct RtmpHandshakeOptions {
    app_version: [u8; 4],
    timestamp: u32,
    random_data: [u8; HANDSHAKE_PACKET_SIZE - 8],
}

impl RtmpHandshakeOptions {
    fn phase1_packet(&self) -> Vec<u8> {
        // エンコードするパケットのサイズは固定であり、入力データ由来ではないため
        // with_capacity() で事前に領域を確保してよい
        let mut packet = Vec::with_capacity(HANDSHAKE_PACKET_SIZE);

        packet.write_u32(self.timestamp);
        packet.write_u32(u32::from_be_bytes(self.app_version));
        packet.write_bytes(&self.random_data);

        packet
    }
}

impl Default for RtmpHandshakeOptions {
    fn default() -> Self {
        Self {
            app_version: APP_VERSION,
            timestamp: TIMESTAMP,
            random_data: RANDOM_DATA,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    #[default]
    P0,
    P1,
    P2,
    Complete,
}

/// RTMP サーバー側のハンドシェイクを処理する構造体
///
/// クライアントから受信した C0/C1/C2 を処理し、S0/S1/S2 の送信データを生成する。
/// 受信データは [`RtmpServerHandshake::feed_recv_buf()`] で渡し、送信データは
/// [`RtmpServerHandshake::send_buf()`] から取得して送信する。
#[derive(Debug)]
pub struct RtmpServerHandshake {
    options: RtmpHandshakeOptions,
    phase: Phase,
    recv_buf: Vec<u8>,
    send_buf: Vec<u8>,
}

impl RtmpServerHandshake {
    /// 新しいサーバー側ハンドシェイク処理を生成する
    pub fn new() -> Self {
        Self {
            options: RtmpHandshakeOptions::default(),
            phase: Phase::P0,
            recv_buf: Vec::new(),
            send_buf: Vec::new(),
        }
    }

    /// 受信したバイト列をハンドシェイク処理に渡す
    ///
    /// ハンドシェイクに必要なデータが揃うまで内部バッファに蓄積する。
    /// 受信した RTMP のバージョンが不正な場合や、クライアントの C2 がサーバーの S1 と
    /// 一致しない場合は [`Error`] を返す。
    pub fn feed_recv_buf(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.recv_buf.extend_from_slice(buf);
        self.handle_recv_buf()?;
        Ok(())
    }

    fn handle_recv_buf(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::P0 => self.handle_phase_p0(),
            Phase::P1 => self.handle_phase_p1(),
            Phase::P2 => self.handle_phase_p2(),
            Phase::Complete => Ok(()),
        }
    }

    fn handle_phase_p0(&mut self) -> Result<(), Error> {
        if self.recv_buf.is_empty() {
            // データが揃っていない
            return Ok(());
        }

        // 受信: C0
        let client_rtmp_version = self.recv_buf.read_u8()?;
        if client_rtmp_version != RTMP_VERSION {
            return Err(Error::invalid_data(format!(
                "invalid RTMP version: expected {RTMP_VERSION}, got {client_rtmp_version}"
            )));
        }

        // 送信: S0
        self.send_buf.write_u8(RTMP_VERSION);

        self.phase = Phase::P1;
        self.handle_phase_p1()
    }

    fn handle_phase_p1(&mut self) -> Result<(), Error> {
        if self.recv_buf.len() < HANDSHAKE_PACKET_SIZE {
            // データが揃っていない
            return Ok(());
        }

        // 受信: C1
        let c1_packet = self.recv_buf.read_bytes(HANDSHAKE_PACKET_SIZE)?;

        // 送信: S1, S2
        self.send_buf.write_bytes(&self.options.phase1_packet());
        self.send_buf.write_bytes(&c1_packet);

        self.phase = Phase::P2;
        self.handle_phase_p2()
    }

    fn handle_phase_p2(&mut self) -> Result<(), Error> {
        if self.recv_buf.len() < HANDSHAKE_PACKET_SIZE {
            // データが揃っていない
            return Ok(());
        }

        // 受信: C2
        let c2_packet = self.recv_buf.read_bytes(HANDSHAKE_PACKET_SIZE)?;
        if self.options.phase1_packet() != c2_packet {
            return Err(Error::invalid_data("C2 packet does not match S1 packet"));
        }

        self.phase = Phase::Complete;
        Ok(())
    }

    /// ハンドシェイク処理で消費しなかった受信データを取得する
    ///
    /// ハンドシェイク完了後に残ったデータは、後続の RTMP 通信で利用する。
    /// 取得後、内部の受信バッファは空になる。
    pub fn take_recv_buf(&mut self) -> Vec<u8> {
        mem::take(&mut self.recv_buf)
    }

    /// 送信待ちのハンドシェイクデータを取得する
    ///
    /// 送信が完了した分は [`Self::advance_send_buf()`] で消費する。
    pub fn send_buf(&self) -> &[u8] {
        &self.send_buf
    }

    /// 送信済みのバイト数を送信バッファから消費する
    ///
    /// `n` が送信バッファの長さを超える場合は、バッファ全体を消費する。
    pub fn advance_send_buf(&mut self, n: usize) {
        let n = n.min(self.send_buf.len());
        self.send_buf.drain(..n); // NOTE: 非効率だけど性能を気にする箇所ではないのでシンプルさを優先している
    }

    /// ハンドシェイクの受信処理が完了したかどうかを返す
    ///
    /// 受信処理が完了した後も、ハンドシェイクに必要なデータを超えた分は
    /// [`Self::take_recv_buf()`] で取得できる。
    pub fn is_recv_complete(&self) -> bool {
        self.phase == Phase::Complete
    }

    /// ハンドシェイクの送信処理が完了したかどうかを返す
    ///
    /// 受信処理が完了していて、かつ送信バッファが空になった場合にのみ `true` を返す。
    pub fn is_send_complete(&self) -> bool {
        self.phase == Phase::Complete && self.send_buf.is_empty()
    }

    /// S0/S1/S2 を送り終えたかを、C2 の受信に関係なく判定する。
    pub(crate) fn has_sent_handshake_bytes(&self) -> bool {
        matches!(self.phase, Phase::P2 | Phase::Complete) && self.send_buf.is_empty()
    }
}

impl Default for RtmpServerHandshake {
    fn default() -> Self {
        Self::new()
    }
}

/// RTMP クライアント側のハンドシェイクを処理する構造体
///
/// サーバーから受信した S0/S1/S2 を処理し、C0/C1/C2 の送信データを生成する。
/// 受信データは [`RtmpClientHandshake::feed_recv_buf()`] で渡し、送信データは
/// [`RtmpClientHandshake::send_buf()`] から取得して送信する。
#[derive(Debug)]
pub struct RtmpClientHandshake {
    options: RtmpHandshakeOptions,
    phase: Phase,
    recv_buf: Vec<u8>,
    send_buf: Vec<u8>,
}

impl RtmpClientHandshake {
    /// 新しいクライアント側ハンドシェイク処理を生成する
    ///
    /// C0 と C1 の送信データが初期化された状態になる。
    pub fn new() -> Self {
        let options = RtmpHandshakeOptions::default();
        let mut send_buf = Vec::new();

        // 送信: C0, C1
        send_buf.push(RTMP_VERSION);
        send_buf.extend_from_slice(&options.phase1_packet());

        Self {
            options,
            phase: Phase::P0,
            recv_buf: Vec::new(),
            send_buf,
        }
    }

    /// 受信したバイト列をハンドシェイク処理に渡す
    ///
    /// ハンドシェイクに必要なデータが揃うまで内部バッファに蓄積する。
    /// 受信した RTMP のバージョンが不正な場合や、サーバーの S2 がクライアントの C1 と
    /// 一致しない場合は [`Error`] を返す。
    pub fn feed_recv_buf(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.recv_buf.extend_from_slice(buf);
        self.handle_recv_buf()?;
        Ok(())
    }

    fn handle_recv_buf(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::P0 => self.handle_phase_p0(),
            Phase::P1 => self.handle_phase_p1(),
            Phase::P2 | Phase::Complete => Ok(()),
        }
    }

    fn handle_phase_p0(&mut self) -> Result<(), Error> {
        if self.recv_buf.is_empty() {
            // データが揃っていない
            return Ok(());
        }

        // 受信: S0
        let server_rtmp_version = self.recv_buf.read_u8()?;
        if server_rtmp_version != RTMP_VERSION {
            return Err(Error::invalid_data(format!(
                "invalid RTMP version: expected {RTMP_VERSION}, got {server_rtmp_version}"
            )));
        }

        self.phase = Phase::P1;
        self.handle_phase_p1()
    }

    fn handle_phase_p1(&mut self) -> Result<(), Error> {
        if self.recv_buf.len() < HANDSHAKE_PACKET_SIZE * 2 {
            // S1 と S2 の両方を受信するまで待つ
            return Ok(());
        }

        // 受信: S1, S2
        let s1_packet = self.recv_buf.read_bytes(HANDSHAKE_PACKET_SIZE)?;
        let s2_packet = self.recv_buf.read_bytes(HANDSHAKE_PACKET_SIZE)?;
        if s2_packet != self.options.phase1_packet() {
            return Err(Error::invalid_data("S2 packet does not match C1 packet"));
        }

        // 送信: C2
        self.send_buf.extend_from_slice(&s1_packet);

        // クライアント側は明示的な Phase 2 への移行は不要
        self.phase = Phase::Complete;
        Ok(())
    }

    /// ハンドシェイク処理で消費しなかった受信データを取得する
    ///
    /// ハンドシェイク完了後に残ったデータは、後続の RTMP 通信で利用する。
    /// 取得後、内部の受信バッファは空になる。
    pub fn take_recv_buf(&mut self) -> Vec<u8> {
        mem::take(&mut self.recv_buf)
    }

    /// 送信待ちのハンドシェイクデータを取得する
    ///
    /// 送信が完了した分は [`Self::advance_send_buf()`] で消費する。
    pub fn send_buf(&self) -> &[u8] {
        &self.send_buf
    }

    /// 送信済みのバイト数を送信バッファから消費する
    ///
    /// `n` が送信バッファの長さを超える場合は、バッファ全体を消費する。
    pub fn advance_send_buf(&mut self, n: usize) {
        let n = n.min(self.send_buf.len());
        self.send_buf.drain(..n); // NOTE: 非効率だけど性能を気にする箇所ではないのでシンプルさを優先している
    }

    /// ハンドシェイクの受信処理が完了したかどうかを返す
    ///
    /// 受信処理が完了した後も、ハンドシェイクに必要なデータを超えた分は
    /// [`Self::take_recv_buf()`] で取得できる。
    pub fn is_recv_complete(&self) -> bool {
        self.phase == Phase::Complete
    }

    /// ハンドシェイクの送信処理が完了したかどうかを返す
    ///
    /// 受信処理が完了していて、かつ送信バッファが空になった場合にのみ `true` を返す。
    pub fn is_send_complete(&self) -> bool {
        self.phase == Phase::Complete && self.send_buf.is_empty()
    }
}

impl Default for RtmpClientHandshake {
    fn default() -> Self {
        Self::new()
    }
}
