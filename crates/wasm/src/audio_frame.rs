use c_api::audio_frame::RtmpAudioFrame;
use c_api::basic_types::{RtmpAudioFormat, RtmpAudioSampleRate};

pub(crate) fn fmt_json_rtmp_audio_frame(
    f: &mut nojson::JsonFormatter<'_, '_>,
    frame: &RtmpAudioFrame,
) -> std::fmt::Result {
    f.object(|f| {
        f.member("timestamp", frame.timestamp_millis())?;
        f.member("format", frame.format().as_str())?;
        f.member("sample_rate", frame.sample_rate().as_str())?;
        f.member("is_8bit_sample", frame.is_8bit_sample())?;
        f.member("is_stereo", frame.is_stereo())?;
        f.member("is_aac_sequence_header", frame.is_aac_sequence_header())?;
        f.member("data", frame.data())
    })
}

/// 音声フレームを JSON に変換する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_to_json(frame: *const RtmpAudioFrame) -> *mut Vec<u8> {
    if frame.is_null() {
        return std::ptr::null_mut();
    }

    let frame = unsafe { &*frame };
    let json = nojson::json(|f| fmt_json_rtmp_audio_frame(f, frame)).to_string();
    Box::into_raw(Box::new(json.into_bytes()))
}

/// JSON から音声フレームを生成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_from_json(
    json_bytes: *const u8,
    json_bytes_len: u32,
) -> *mut RtmpAudioFrame {
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

    let Ok(frame) = parse_json_rtmp_audio_frame(raw_json.value()) else {
        return std::ptr::null_mut();
    };

    Box::into_raw(Box::new(frame))
}

fn parse_json_rtmp_audio_frame(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpAudioFrame, nojson::JsonParseError> {
    let timestamp: u32 = value.to_member("timestamp")?.required()?.try_into()?;
    let format = parse_audio_format(value.to_member("format")?.required()?)?;
    let sample_rate = parse_audio_sample_rate(value.to_member("sample_rate")?.required()?)?;
    let is_8bit_sample: bool = value.to_member("is_8bit_sample")?.required()?.try_into()?;
    let is_stereo: bool = value.to_member("is_stereo")?.required()?.try_into()?;
    let is_aac_sequence_header: bool = value
        .to_member("is_aac_sequence_header")?
        .required()?
        .try_into()?;
    let data: Vec<u8> = value.to_member("data")?.required()?.try_into()?;

    Ok(RtmpAudioFrame::new(
        timestamp,
        format,
        sample_rate,
        is_8bit_sample,
        is_stereo,
        is_aac_sequence_header,
        data,
    ))
}

fn parse_audio_format(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpAudioFormat, nojson::JsonParseError> {
    let value_str = value.to_unquoted_string_str()?;
    match value_str.as_ref() {
        "adpcm" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_ADPCM),
        "mp3" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_MP3),
        "linear_pcm_little_endian" => {
            Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN)
        }
        "nellymoser_16khz_mono" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO),
        "nellymoser_8khz_mono" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO),
        "nellymoser" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER),
        "g711_alaw_logarithmic_pcm" => {
            Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_G711_ALAW_LOGARITHMIC_PCM)
        }
        "g711_mulaw_logarithmic_pcm" => {
            Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_G711_MULAW_LOGARITHMIC_PCM)
        }
        "aac" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_AAC),
        "speex" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_SPEEX),
        "mp3_8khz" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_MP3_8KHZ),
        "device_specific_sound" => Ok(RtmpAudioFormat::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND),
        _ => Err(value.invalid("unknown audio format")),
    }
}

fn parse_audio_sample_rate(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<RtmpAudioSampleRate, nojson::JsonParseError> {
    let value_str = value.to_unquoted_string_str()?;
    match value_str.as_ref() {
        "5khz" => Ok(RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ5),
        "11khz" => Ok(RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ11),
        "22khz" => Ok(RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ22),
        "44khz" => Ok(RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ44),
        _ => Err(value.invalid("unknown audio sample rate")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_frame_json_roundtrip() {
        let input = br#"{"timestamp":0,"format":"aac","sample_rate":"44khz","is_8bit_sample":false,"is_stereo":true,"is_aac_sequence_header":false,"data":[1,2,3]}"#;
        let frame = unsafe { rtmp_audio_frame_from_json(input.as_ptr(), input.len() as u32) };
        assert!(!frame.is_null());

        let json = unsafe { rtmp_audio_frame_to_json(frame) };
        assert!(!json.is_null());
        let json_text = String::from_utf8(unsafe { (*json).clone() }).expect("valid utf-8");
        assert!(json_text.contains("\"format\":\"aac\""));
        assert!(json_text.contains("\"sample_rate\":\"44khz\""));
        assert!(json_text.contains("\"data\":[1,2,3]"));

        unsafe {
            let _ = Box::from_raw(json);
            c_api::audio_frame::rtmp_audio_frame_free(frame);
        }
    }
}
