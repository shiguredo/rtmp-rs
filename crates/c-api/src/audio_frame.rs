use shiguredo_rtmp::{AudioFrame, RtmpTimestamp};

use crate::basic_types::{RtmpAudioFormat, RtmpAudioSampleRate};
use crate::error::RtmpError;
use crate::util::{ptr_or_null, slice_arg, write_box_out};

/// 音声フレームの opaque ハンドル
pub struct RtmpAudioFrame {
    inner: AudioFrame,
}

impl RtmpAudioFrame {
    pub fn new(
        timestamp_millis: u32,
        format: RtmpAudioFormat,
        sample_rate: RtmpAudioSampleRate,
        is_8bit_sample: bool,
        is_stereo: bool,
        is_aac_sequence_header: bool,
        data: Vec<u8>,
    ) -> Self {
        Self {
            inner: AudioFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp_millis),
                format: format.into(),
                sample_rate: sample_rate.into(),
                is_8bit_sample,
                is_stereo,
                is_aac_sequence_header,
                data,
            },
        }
    }

    pub fn timestamp_millis(&self) -> u32 {
        self.inner.timestamp.as_millis()
    }

    pub fn format(&self) -> RtmpAudioFormat {
        self.inner.format.into()
    }

    pub fn sample_rate(&self) -> RtmpAudioSampleRate {
        self.inner.sample_rate.into()
    }

    pub fn is_8bit_sample(&self) -> bool {
        self.inner.is_8bit_sample
    }

    pub fn is_stereo(&self) -> bool {
        self.inner.is_stereo
    }

    pub fn is_aac_sequence_header(&self) -> bool {
        self.inner.is_aac_sequence_header
    }

    pub fn data(&self) -> &[u8] {
        &self.inner.data
    }

    pub(crate) fn from_inner(inner: AudioFrame) -> Self {
        Self { inner }
    }

    pub(crate) fn clone_inner(&self) -> AudioFrame {
        self.inner.clone()
    }
}

/// 音声フレームを作成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_new(
    timestamp_millis: u32,
    format: RtmpAudioFormat,
    sample_rate: RtmpAudioSampleRate,
    is_8bit_sample: bool,
    is_stereo: bool,
    is_aac_sequence_header: bool,
    data: *const u8,
    data_len: usize,
    out: *mut *mut RtmpAudioFrame,
) -> RtmpError {
    let data = match unsafe { slice_arg(data, data_len, "data") } {
        Ok(data) => data.to_vec(),
        Err(error) => return error,
    };
    let frame = RtmpAudioFrame::new(
        timestamp_millis,
        format,
        sample_rate,
        is_8bit_sample,
        is_stereo,
        is_aac_sequence_header,
        data,
    );
    match unsafe { write_box_out(out, frame) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 音声フレームを解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_free(frame: *mut RtmpAudioFrame) {
    if !frame.is_null() {
        let _ = unsafe { Box::from_raw(frame) };
    }
}

/// 音声フレームのタイムスタンプを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_timestamp_millis(frame: *const RtmpAudioFrame) -> u32 {
    if frame.is_null() {
        return 0;
    }
    unsafe { (&*frame).timestamp_millis() }
}

/// 音声フレームのフォーマットを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_format(frame: *const RtmpAudioFrame) -> RtmpAudioFormat {
    if frame.is_null() {
        return RtmpAudioFormat::RTMP_AUDIO_FORMAT_AAC;
    }
    unsafe { (&*frame).format() }
}

/// 音声フレームのサンプルレートを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_sample_rate(
    frame: *const RtmpAudioFrame,
) -> RtmpAudioSampleRate {
    if frame.is_null() {
        return RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ44;
    }
    unsafe { (&*frame).sample_rate() }
}

/// 8bit サンプルかを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_is_8bit_sample(frame: *const RtmpAudioFrame) -> bool {
    if frame.is_null() {
        return false;
    }
    unsafe { (&*frame).is_8bit_sample() }
}

/// ステレオかを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_is_stereo(frame: *const RtmpAudioFrame) -> bool {
    if frame.is_null() {
        return false;
    }
    unsafe { (&*frame).is_stereo() }
}

/// AAC sequence header かを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_is_aac_sequence_header(
    frame: *const RtmpAudioFrame,
) -> bool {
    if frame.is_null() {
        return false;
    }
    unsafe { (&*frame).is_aac_sequence_header() }
}

/// ペイロード先頭ポインタを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_data_ptr(frame: *const RtmpAudioFrame) -> *const u8 {
    if frame.is_null() {
        return std::ptr::null();
    }
    unsafe { ptr_or_null((&*frame).data()) }
}

/// ペイロード長を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_audio_frame_data_len(frame: *const RtmpAudioFrame) -> usize {
    if frame.is_null() {
        return 0;
    }
    unsafe { (&*frame).data().len() }
}
