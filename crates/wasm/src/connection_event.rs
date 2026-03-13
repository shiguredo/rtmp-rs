use c_api::basic_types::RtmpConnectionEventKind;
use c_api::connection_event::RtmpConnectionEvent;

/// RTMP イベントを JSON に変換する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_connection_event_to_json(
    event: *const RtmpConnectionEvent,
) -> *mut Vec<u8> {
    if event.is_null() {
        return std::ptr::null_mut();
    }

    let event = unsafe { &*event };
    let json = nojson::json(|f| fmt_json_rtmp_connection_event(f, event)).to_string();
    Box::into_raw(Box::new(json.into_bytes()))
}

fn fmt_json_rtmp_connection_event(
    f: &mut nojson::JsonFormatter<'_, '_>,
    event: &RtmpConnectionEvent,
) -> std::fmt::Result {
    let kind = event.kind();
    f.object(|f| {
        f.member("kind", kind.as_str())?;

        match kind {
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_PUBLISH_REQUESTED
            | RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_PLAY_REQUESTED => {
                if let Some(app) = event.app() {
                    f.member("app", app.to_str().expect("event strings are valid UTF-8"))?;
                }
                if let Some(tc_url) = event.tc_url() {
                    f.member(
                        "tc_url",
                        tc_url.to_str().expect("event strings are valid UTF-8"),
                    )?;
                }
                if let Some(stream_name) = event.stream_name() {
                    f.member(
                        "stream_name",
                        stream_name.to_str().expect("event strings are valid UTF-8"),
                    )?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_AUDIO_RECEIVED => {
                if let Some(frame) = event.audio_frame() {
                    f.member(
                        "frame",
                        nojson::json(|f| crate::audio_frame::fmt_json_rtmp_audio_frame(f, frame)),
                    )?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_VIDEO_RECEIVED => {
                if let Some(frame) = event.video_frame() {
                    f.member(
                        "frame",
                        nojson::json(|f| crate::video_frame::fmt_json_rtmp_video_frame(f, frame)),
                    )?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_STATE_CHANGED => {
                if let Some(state) = event.state() {
                    f.member("state", state.as_str())?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_DISCONNECTED_BY_PEER => {
                if let Some(reason) = event.reason() {
                    f.member(
                        "reason",
                        reason.to_str().expect("event strings are valid UTF-8"),
                    )?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_COMMAND_IGNORED
            | RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_MESSAGE_IGNORED
            | RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_USER_CONTROL_EVENT_IGNORED => {
                if let Some(name) = event.name() {
                    f.member(
                        "name",
                        name.to_str().expect("event strings are valid UTF-8"),
                    )?;
                }
                if let Some(detail) = event.detail() {
                    f.member(
                        "detail",
                        detail.to_str().expect("event strings are valid UTF-8"),
                    )?;
                }
            }
            RtmpConnectionEventKind::RTMP_CONNECTION_EVENT_KIND_NONE => {}
        }

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_json_contains_kind_and_state() {
        let mut connection = std::ptr::null_mut();
        let status = unsafe {
            c_api::publish_client::rtmp_publish_client_connection_new(
                c"rtmp://localhost/live/test".as_ptr(),
                &mut connection,
            )
        };
        assert_eq!(status, c_api::error::RtmpError::RTMP_ERROR_OK);
        assert!(!connection.is_null());

        let mut event = std::ptr::null_mut();
        let status = unsafe {
            c_api::publish_client::rtmp_publish_client_connection_next_event(connection, &mut event)
        };
        assert_eq!(status, c_api::error::RtmpError::RTMP_ERROR_OK);
        assert!(!event.is_null());

        let json = unsafe { rtmp_connection_event_to_json(event) };
        assert!(!json.is_null());
        let json_text = String::from_utf8(unsafe { (*json).clone() }).expect("valid utf-8");
        assert!(json_text.contains("\"kind\":\"state_changed\""));
        assert!(json_text.contains("\"state\":\"handshaking\""));

        unsafe {
            let _ = Box::from_raw(json);
            c_api::connection_event::rtmp_connection_event_free(event);
            c_api::publish_client::rtmp_publish_client_connection_free(connection);
        }
    }
}
