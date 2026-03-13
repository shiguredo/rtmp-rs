use c_api::basic_types::{RtmpAvcPacketType, RtmpVideoCodec, RtmpVideoFrameType};
use c_api::video_frame::RtmpVideoFrame;
use nojson::JsonValueKind;

pub(crate) fn fmt_json_rtmp_video_frame(
    f: &mut nojson::JsonFormatter<'_, '_>,
    frame: &RtmpVideoFrame,
) -> std::fmt::Result {
    f.object(|f| {
        f.member("timestamp", frame.timestamp_millis())?;
        f.member(
            "composition_timestamp_offset",
            frame.composition_timestamp_offset_millis(),
        )?;
        f.member("frame_type", frame.frame_type().as_str())?;
        f.member("codec", frame.codec().as_str())?;
        f.member(
            "avc_packet_type",
            frame
                .avc_packet_type()
                .map(|packet_type| packet_type.as_str()),
        )?;
        f.member("data", frame.data())
    })
}

/// 映像フレームを JSON に変換する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_to_json(frame: *const RtmpVideoFrame) -> *mut Vec<u8> {
    if frame.is_null() {
        return std::ptr::null_mut();
    }

    let frame = unsafe { &*frame };
    let json = nojson::json(|f| fmt_json_rtmp_video_frame(f, frame)).to_string();
    Box::into_raw(Box::new(json.into_bytes()))
}

/// JSON から映像フレームを生成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_from_json(
    json_bytes: *const u8,
    json_bytes_len: u32,
) -> *mut RtmpVideoFrame {
    if json_bytes.is_null() {
        return std::ptr::null_mut();
    }

    let Ok(json_text) = std::str::from_utf8(unsafe {
        std::slice::from_raw_parts(json_bytes, json_bytes_len as usize)
    }) else {
        return std::ptr::null_mut();
    };

    let Ok(raw_json) = nojson::RawJson::parse(json_text) else {
        return std::ptr::null_mut();
    };

    let Ok(frame) = parse_json_rtmp_video_frame(raw_json.value()) else {
        return std::ptr::null_mut();
    };

    Box::into_raw(Box::new(frame))
}

fn parse_json_rtmp_video_frame(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpVideoFrame, nojson::JsonParseError> {
    let timestamp: u32 = value.to_member("timestamp")?.required()?.try_into()?;
    let composition_timestamp_offset: i32 = value
        .to_member("composition_timestamp_offset")?
        .required()?
        .try_into()?;
    let frame_type = parse_video_frame_type(value.to_member("frame_type")?.required()?)?;
    let codec = parse_video_codec(value.to_member("codec")?.required()?)?;
    let avc_packet_type = match value.to_member("avc_packet_type")?.optional() {
        Some(avc_packet_type) if avc_packet_type.kind() == JsonValueKind::Null => None,
        Some(avc_packet_type) => Some(parse_avc_packet_type(avc_packet_type)?),
        None => None,
    };
    let data: Vec<u8> = value.to_member("data")?.required()?.try_into()?;

    Ok(RtmpVideoFrame::new(
        timestamp,
        composition_timestamp_offset,
        frame_type,
        codec,
        avc_packet_type,
        data,
    ))
}

fn parse_video_frame_type(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpVideoFrameType, nojson::JsonParseError> {
    let value_str = value.to_unquoted_string_str()?;
    match value_str.as_ref() {
        "key_frame" => Ok(RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME),
        "inter_frame" => Ok(RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_INTER_FRAME),
        "disposable_inter_frame" => {
            Ok(RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME)
        }
        "generated_key_frame" => Ok(RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME),
        "video_info_or_command_frame" => {
            Ok(RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME)
        }
        _ => Err(value.invalid("unknown video frame type")),
    }
}

fn parse_video_codec(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpVideoCodec, nojson::JsonParseError> {
    let value_str = value.to_unquoted_string_str()?;
    match value_str.as_ref() {
        "jpeg" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_JPEG),
        "h263" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_H263),
        "screen_video" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO),
        "vp6" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_VP6),
        "vp6_with_alpha" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_VP6_WITH_ALPHA),
        "screen_video_v2" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2),
        "avc" => Ok(RtmpVideoCodec::RTMP_VIDEO_CODEC_AVC),
        _ => Err(value.invalid("unknown video codec")),
    }
}

fn parse_avc_packet_type(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpAvcPacketType, nojson::JsonParseError> {
    let value_str = value.to_unquoted_string_str()?;
    match value_str.as_ref() {
        "sequence_header" => Ok(RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER),
        "nal_unit" => Ok(RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_NAL_UNIT),
        "end_of_sequence" => Ok(RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE),
        _ => Err(value.invalid("unknown avc packet type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_frame_json_roundtrip() {
        let input = br#"{"timestamp":0,"composition_timestamp_offset":0,"frame_type":"key_frame","codec":"avc","avc_packet_type":"nal_unit","data":[1,2,3,4]}"#;
        let frame = unsafe { rtmp_video_frame_from_json(input.as_ptr(), input.len() as u32) };
        assert!(!frame.is_null());

        let json = unsafe { rtmp_video_frame_to_json(frame) };
        assert!(!json.is_null());
        let json_text = String::from_utf8(unsafe { (*json).clone() }).expect("valid utf-8");
        assert!(json_text.contains("\"frame_type\":\"key_frame\""));
        assert!(json_text.contains("\"codec\":\"avc\""));
        assert!(json_text.contains("\"avc_packet_type\":\"nal_unit\""));

        unsafe {
            let _ = Box::from_raw(json);
            c_api::video_frame::rtmp_video_frame_free(frame);
        }
    }
}
