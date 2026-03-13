use shiguredo_rtmp::{RtmpTimestamp, RtmpTimestampDelta, VideoFrame};

use crate::basic_types::{RtmpAvcPacketType, RtmpVideoCodec, RtmpVideoFrameType};
use crate::error::RtmpError;
use crate::util::{ptr_or_null, slice_arg, write_box_out};

/// 映像フレームの opaque ハンドル
pub struct RtmpVideoFrame {
    inner: VideoFrame,
}

impl RtmpVideoFrame {
    pub fn new(
        timestamp_millis: u32,
        composition_timestamp_offset_millis: i32,
        frame_type: RtmpVideoFrameType,
        codec: RtmpVideoCodec,
        avc_packet_type: Option<RtmpAvcPacketType>,
        data: Vec<u8>,
    ) -> Self {
        Self {
            inner: VideoFrame {
                timestamp: RtmpTimestamp::from_millis(timestamp_millis),
                composition_timestamp_offset: RtmpTimestampDelta::from_millis(
                    composition_timestamp_offset_millis,
                ),
                frame_type: frame_type.into(),
                codec: codec.into(),
                avc_packet_type: avc_packet_type.map(Into::into),
                data,
            },
        }
    }

    pub fn timestamp_millis(&self) -> u32 {
        self.inner.timestamp.as_millis()
    }

    pub fn composition_timestamp_offset_millis(&self) -> i32 {
        self.inner.composition_timestamp_offset.as_millis()
    }

    pub fn frame_type(&self) -> RtmpVideoFrameType {
        self.inner.frame_type.into()
    }

    pub fn codec(&self) -> RtmpVideoCodec {
        self.inner.codec.into()
    }

    pub fn avc_packet_type(&self) -> Option<RtmpAvcPacketType> {
        self.inner.avc_packet_type.map(Into::into)
    }

    pub fn data(&self) -> &[u8] {
        &self.inner.data
    }

    pub(crate) fn from_inner(inner: VideoFrame) -> Self {
        Self { inner }
    }

    pub(crate) fn clone_inner(&self) -> VideoFrame {
        self.inner.clone()
    }
}

/// 映像フレームを作成する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_new(
    timestamp_millis: u32,
    composition_timestamp_offset_millis: i32,
    frame_type: RtmpVideoFrameType,
    codec: RtmpVideoCodec,
    has_avc_packet_type: bool,
    avc_packet_type: RtmpAvcPacketType,
    data: *const u8,
    data_len: usize,
    out: *mut *mut RtmpVideoFrame,
) -> RtmpError {
    let data = match unsafe { slice_arg(data, data_len, "data") } {
        Ok(data) => data.to_vec(),
        Err(error) => return error,
    };
    let avc_packet_type = if has_avc_packet_type {
        Some(avc_packet_type)
    } else {
        None
    };
    let frame = RtmpVideoFrame::new(
        timestamp_millis,
        composition_timestamp_offset_millis,
        frame_type,
        codec,
        avc_packet_type,
        data,
    );
    match unsafe { write_box_out(out, frame) } {
        Ok(()) => RtmpError::RTMP_ERROR_OK,
        Err(error) => error,
    }
}

/// 映像フレームを解放する
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_free(frame: *mut RtmpVideoFrame) {
    if !frame.is_null() {
        let _ = unsafe { Box::from_raw(frame) };
    }
}

/// 映像フレームのタイムスタンプを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_timestamp_millis(frame: *const RtmpVideoFrame) -> u32 {
    if frame.is_null() {
        return 0;
    }
    unsafe { (&*frame).timestamp_millis() }
}

/// 合成時間オフセットを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_composition_timestamp_offset_millis(
    frame: *const RtmpVideoFrame,
) -> i32 {
    if frame.is_null() {
        return 0;
    }
    unsafe { (&*frame).composition_timestamp_offset_millis() }
}

/// フレーム種別を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_frame_type(
    frame: *const RtmpVideoFrame,
) -> RtmpVideoFrameType {
    if frame.is_null() {
        return RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME;
    }
    unsafe { (&*frame).frame_type() }
}

/// コーデックを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_codec(frame: *const RtmpVideoFrame) -> RtmpVideoCodec {
    if frame.is_null() {
        return RtmpVideoCodec::RTMP_VIDEO_CODEC_AVC;
    }
    unsafe { (&*frame).codec() }
}

/// AVC パケット種別を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_avc_packet_type(
    frame: *const RtmpVideoFrame,
    out: *mut RtmpAvcPacketType,
) -> bool {
    if frame.is_null() || out.is_null() {
        return false;
    }
    let Some(packet_type) = (unsafe { &*frame }).avc_packet_type() else {
        return false;
    };
    unsafe {
        std::ptr::write(out, packet_type);
    }
    true
}

/// ペイロード先頭ポインタを返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_data_ptr(frame: *const RtmpVideoFrame) -> *const u8 {
    if frame.is_null() {
        return std::ptr::null();
    }
    unsafe { ptr_or_null((&*frame).data()) }
}

/// ペイロード長を返す
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rtmp_video_frame_data_len(frame: *const RtmpVideoFrame) -> usize {
    if frame.is_null() {
        return 0;
    }
    unsafe { (&*frame).data().len() }
}
