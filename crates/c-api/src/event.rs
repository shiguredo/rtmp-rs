use std::ffi::{CString, c_char};

use shiguredo_rtmp::RtmpConnectionEvent;

use crate::types::{
    RtmpCAudioFormat, RtmpCAudioSampleRate, RtmpCAvcPacketType, RtmpCConnectionState,
    RtmpCVideoCodec, RtmpCVideoFrameType,
};

/// イベントの種類
#[repr(C)]
#[expect(non_camel_case_types)]
pub enum RtmpCEventKind {
    /// 音声フレームを受信した
    RTMP_EVENT_AUDIO_RECEIVED,

    /// 映像フレームを受信した
    RTMP_EVENT_VIDEO_RECEIVED,

    /// コネクションの状態が変更された
    RTMP_EVENT_STATE_CHANGED,

    /// 相手から切断された
    RTMP_EVENT_DISCONNECTED_BY_PEER,

    /// イベントなし（next_event が None のとき）
    RTMP_EVENT_NONE,
}

/// C 用のイベント構造体
///
/// kind フィールドに応じて、対応するフィールドが有効になる:
/// - RTMP_EVENT_AUDIO_RECEIVED: audio_* フィールド
/// - RTMP_EVENT_VIDEO_RECEIVED: video_* フィールド
/// - RTMP_EVENT_STATE_CHANGED: state フィールド
/// - RTMP_EVENT_DISCONNECTED_BY_PEER: reason フィールド
#[repr(C)]
pub struct RtmpCEvent {
    pub kind: RtmpCEventKind,

    // AudioReceived 用
    pub audio_timestamp_millis: u32,
    pub audio_format: RtmpCAudioFormat,
    pub audio_sample_rate: RtmpCAudioSampleRate,
    pub audio_is_8bit_sample: bool,
    pub audio_is_stereo: bool,
    pub audio_is_aac_sequence_header: bool,
    pub audio_data: *const u8,
    pub audio_data_len: usize,

    // VideoReceived 用
    pub video_timestamp_millis: u32,
    pub video_composition_timestamp_offset_millis: i32,
    pub video_frame_type: RtmpCVideoFrameType,
    pub video_codec: RtmpCVideoCodec,
    pub video_has_avc_packet_type: bool,
    pub video_avc_packet_type: RtmpCAvcPacketType,
    pub video_data: *const u8,
    pub video_data_len: usize,

    // StateChanged 用
    pub state: RtmpCConnectionState,

    // DisconnectedByPeer 用
    pub reason: *const c_char,

    // 内部: データの所有権を保持するためのフィールド（C 側からはアクセスしない）
    _owned_audio_data: Option<Vec<u8>>,
    _owned_video_data: Option<Vec<u8>>,
    _owned_reason: Option<CString>,
}

impl RtmpCEvent {
    /// イベントなしを表す RtmpCEvent を生成する
    pub(crate) fn none() -> Self {
        Self {
            kind: RtmpCEventKind::RTMP_EVENT_NONE,
            audio_timestamp_millis: 0,
            audio_format: RtmpCAudioFormat::RTMP_AUDIO_FORMAT_AAC,
            audio_sample_rate: RtmpCAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_44KHZ,
            audio_is_8bit_sample: false,
            audio_is_stereo: false,
            audio_is_aac_sequence_header: false,
            audio_data: std::ptr::null(),
            audio_data_len: 0,
            video_timestamp_millis: 0,
            video_composition_timestamp_offset_millis: 0,
            video_frame_type: RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME,
            video_codec: RtmpCVideoCodec::RTMP_VIDEO_CODEC_AVC,
            video_has_avc_packet_type: false,
            video_avc_packet_type: RtmpCAvcPacketType::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER,
            video_data: std::ptr::null(),
            video_data_len: 0,
            state: RtmpCConnectionState::RTMP_STATE_HANDSHAKING,
            reason: std::ptr::null(),
            _owned_audio_data: None,
            _owned_video_data: None,
            _owned_reason: None,
        }
    }

    /// RtmpConnectionEvent から RtmpCEvent を生成する
    pub(crate) fn from_event(event: RtmpConnectionEvent) -> Self {
        let mut result = Self::none();

        match event {
            RtmpConnectionEvent::AudioReceived(frame) => {
                result.kind = RtmpCEventKind::RTMP_EVENT_AUDIO_RECEIVED;
                result.audio_timestamp_millis = frame.timestamp.as_millis();
                result.audio_format = frame.format.into();
                result.audio_sample_rate = frame.sample_rate.into();
                result.audio_is_8bit_sample = frame.is_8bit_sample;
                result.audio_is_stereo = frame.is_stereo;
                result.audio_is_aac_sequence_header = frame.is_aac_sequence_header;
                result._owned_audio_data = Some(frame.data);
                let data = result._owned_audio_data.as_ref().unwrap();
                result.audio_data = data.as_ptr();
                result.audio_data_len = data.len();
            }
            RtmpConnectionEvent::VideoReceived(frame) => {
                result.kind = RtmpCEventKind::RTMP_EVENT_VIDEO_RECEIVED;
                result.video_timestamp_millis = frame.timestamp.as_millis();
                result.video_composition_timestamp_offset_millis =
                    frame.composition_timestamp_offset.as_millis();
                result.video_frame_type = frame.frame_type.into();
                result.video_codec = frame.codec.into();
                if let Some(avc_packet_type) = frame.avc_packet_type {
                    result.video_has_avc_packet_type = true;
                    result.video_avc_packet_type = avc_packet_type.into();
                }
                result._owned_video_data = Some(frame.data);
                let data = result._owned_video_data.as_ref().unwrap();
                result.video_data = data.as_ptr();
                result.video_data_len = data.len();
            }
            RtmpConnectionEvent::StateChanged(state) => {
                result.kind = RtmpCEventKind::RTMP_EVENT_STATE_CHANGED;
                result.state = state.into();
            }
            RtmpConnectionEvent::DisconnectedByPeer { reason } => {
                result.kind = RtmpCEventKind::RTMP_EVENT_DISCONNECTED_BY_PEER;
                let c_reason = CString::new(reason).unwrap_or_default();
                result.reason = c_reason.as_ptr();
                result._owned_reason = Some(c_reason);
            }
            // クライアント側では PublishRequested / PlayRequested は発生しない
            // CommandIgnored / MessageIgnored / UserControlEventIgnored はデバッグ用なので省略
            _ => {}
        }

        result
    }
}

/// RtmpCEvent を解放する
///
/// NULL ポインタが渡された場合は何もしない
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_event_free(event: *mut RtmpCEvent) {
    if !event.is_null() {
        let _ = unsafe { Box::from_raw(event) };
    }
}
