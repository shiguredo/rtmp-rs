use std::ffi::{CStr, CString, c_char};

use shiguredo_rtmp::RtmpConnectionEvent as InnerConnectionEvent;

use crate::audio_frame::RtmpAudioFrame;
use crate::basic_types::{RtmpConnectionEventKind, RtmpConnectionState};
use crate::util::cstring_lossy;
use crate::video_frame::RtmpVideoFrame;

struct RequestInfo {
    app: CString,
    tc_url: CString,
    stream_name: CString,
}

struct NamedDetailInfo {
    name: CString,
    detail: CString,
}

enum EventData {
    PublishRequested(RequestInfo),
    PlayRequested(RequestInfo),
    AudioReceived(RtmpAudioFrame),
    VideoReceived(RtmpVideoFrame),
    StateChanged(RtmpConnectionState),
    DisconnectedByPeer { reason: CString },
    CommandIgnored(NamedDetailInfo),
    MessageIgnored(NamedDetailInfo),
    UserControlEventIgnored(NamedDetailInfo),
}

/// RTMP イベントの opaque ハンドル
pub struct RtmpConnectionEvent {
    inner: EventData,
}

impl RtmpConnectionEvent {
    pub(crate) fn from_inner(event: InnerConnectionEvent) -> Self {
        let inner = match event {
            InnerConnectionEvent::PublishRequested {
                app,
                tc_url,
                stream_name,
            } => EventData::PublishRequested(RequestInfo {
                app: cstring_lossy(app),
                tc_url: cstring_lossy(tc_url),
                stream_name: cstring_lossy(stream_name),
            }),
            InnerConnectionEvent::PlayRequested {
                app,
                tc_url,
                stream_name,
            } => EventData::PlayRequested(RequestInfo {
                app: cstring_lossy(app),
                tc_url: cstring_lossy(tc_url),
                stream_name: cstring_lossy(stream_name),
            }),
            InnerConnectionEvent::AudioReceived(frame) => {
                EventData::AudioReceived(RtmpAudioFrame::from_inner(frame))
            }
            InnerConnectionEvent::VideoReceived(frame) => {
                EventData::VideoReceived(RtmpVideoFrame::from_inner(frame))
            }
            InnerConnectionEvent::StateChanged(state) => EventData::StateChanged(state.into()),
            InnerConnectionEvent::DisconnectedByPeer { reason } => EventData::DisconnectedByPeer {
                reason: cstring_lossy(reason),
            },
            InnerConnectionEvent::CommandIgnored { name, detail } => {
                EventData::CommandIgnored(NamedDetailInfo {
                    name: cstring_lossy(name),
                    detail: cstring_lossy(detail),
                })
            }
            InnerConnectionEvent::MessageIgnored { name, detail } => {
                EventData::MessageIgnored(NamedDetailInfo {
                    name: cstring_lossy(name),
                    detail: cstring_lossy(detail),
                })
            }
            InnerConnectionEvent::UserControlEventIgnored { name, detail } => {
                EventData::UserControlEventIgnored(NamedDetailInfo {
                    name: cstring_lossy(name),
                    detail: cstring_lossy(detail),
                })
            }
        };
        Self { inner }
    }

    pub fn kind(&self) -> RtmpConnectionEventKind {
        match &self.inner {
            EventData::PublishRequested(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_PUBLISH_REQUESTED
            }
            EventData::PlayRequested(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_PLAY_REQUESTED
            }
            EventData::AudioReceived(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_AUDIO_RECEIVED
            }
            EventData::VideoReceived(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_VIDEO_RECEIVED
            }
            EventData::StateChanged(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_STATE_CHANGED
            }
            EventData::DisconnectedByPeer { .. } => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_DISCONNECTED_BY_PEER
            }
            EventData::CommandIgnored(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_COMMAND_IGNORED
            }
            EventData::MessageIgnored(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_MESSAGE_IGNORED
            }
            EventData::UserControlEventIgnored(_) => {
                RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_USER_CONTROL_EVENT_IGNORED
            }
        }
    }

    pub fn app(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::PublishRequested(info) | EventData::PlayRequested(info) => {
                Some(info.app.as_c_str())
            }
            _ => None,
        }
    }

    pub fn tc_url(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::PublishRequested(info) | EventData::PlayRequested(info) => {
                Some(info.tc_url.as_c_str())
            }
            _ => None,
        }
    }

    pub fn stream_name(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::PublishRequested(info) | EventData::PlayRequested(info) => {
                Some(info.stream_name.as_c_str())
            }
            _ => None,
        }
    }

    pub fn state(&self) -> Option<RtmpConnectionState> {
        match &self.inner {
            EventData::StateChanged(state) => Some(*state),
            _ => None,
        }
    }

    pub fn audio_frame(&self) -> Option<&RtmpAudioFrame> {
        match &self.inner {
            EventData::AudioReceived(frame) => Some(frame),
            _ => None,
        }
    }

    pub fn video_frame(&self) -> Option<&RtmpVideoFrame> {
        match &self.inner {
            EventData::VideoReceived(frame) => Some(frame),
            _ => None,
        }
    }

    pub fn reason(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::DisconnectedByPeer { reason } => Some(reason.as_c_str()),
            _ => None,
        }
    }

    pub fn name(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::CommandIgnored(info)
            | EventData::MessageIgnored(info)
            | EventData::UserControlEventIgnored(info) => Some(info.name.as_c_str()),
            _ => None,
        }
    }

    pub fn detail(&self) -> Option<&CStr> {
        match &self.inner {
            EventData::CommandIgnored(info)
            | EventData::MessageIgnored(info)
            | EventData::UserControlEventIgnored(info) => Some(info.detail.as_c_str()),
            _ => None,
        }
    }
}

/// イベントを解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_free(event: *mut RtmpConnectionEvent) {
    if !event.is_null() {
        let _ = unsafe { Box::from_raw(event) };
    }
}

/// イベント種別を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_kind(
    event: *const RtmpConnectionEvent,
) -> RtmpConnectionEventKind {
    if event.is_null() {
        return RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_NONE;
    }
    unsafe { (&*event).kind() }
}

/// Publish / Play 系イベントの app を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_app(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .app()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}

/// Publish / Play 系イベントの tc_url を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_tc_url(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .tc_url()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}

/// Publish / Play 系イベントの stream_name を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_stream_name(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .stream_name()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}

/// StateChanged イベントの状態を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_state(
    event: *const RtmpConnectionEvent,
    out: *mut RtmpConnectionState,
) -> bool {
    if event.is_null() || out.is_null() {
        return false;
    }
    let Some(state) = (unsafe { (&*event).state() }) else {
        return false;
    };
    unsafe {
        std::ptr::write(out, state);
    }
    true
}

/// AudioReceived イベントの音声フレームを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_audio_frame(
    event: *const RtmpConnectionEvent,
) -> *const RtmpAudioFrame {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .audio_frame()
            .map_or(std::ptr::null(), |frame| frame as *const RtmpAudioFrame)
    }
}

/// VideoReceived イベントの映像フレームを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_video_frame(
    event: *const RtmpConnectionEvent,
) -> *const RtmpVideoFrame {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .video_frame()
            .map_or(std::ptr::null(), |frame| frame as *const RtmpVideoFrame)
    }
}

/// 切断理由を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_reason(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .reason()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}

/// ignored 系イベントの name を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_name(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .name()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}

/// ignored 系イベントの detail を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_detail(
    event: *const RtmpConnectionEvent,
) -> *const c_char {
    if event.is_null() {
        return std::ptr::null();
    }
    unsafe {
        (&*event)
            .detail()
            .map_or(std::ptr::null(), |value| value.as_ptr())
    }
}
