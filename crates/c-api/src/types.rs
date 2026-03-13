use shiguredo_rtmp::{
    AudioFormat, AudioSampleRate, AvcPacketType, RtmpConnectionState, VideoCodec, VideoFrameType,
};

/// RTMP コネクションの状態
#[repr(C)]
#[expect(non_camel_case_types)]
pub enum RtmpCConnectionState {
    RTMP_STATE_HANDSHAKING = 0,
    RTMP_STATE_CONNECTING,
    RTMP_STATE_CONNECTED,
    RTMP_STATE_MEDIA_STREAM_CREATED,
    RTMP_STATE_PUBLISH_PENDING,
    RTMP_STATE_PUBLISHING,
    RTMP_STATE_PLAY_PENDING,
    RTMP_STATE_PLAYING,
    RTMP_STATE_DISCONNECTING,
}

impl From<RtmpConnectionState> for RtmpCConnectionState {
    fn from(state: RtmpConnectionState) -> Self {
        match state {
            RtmpConnectionState::Handshaking => Self::RTMP_STATE_HANDSHAKING,
            RtmpConnectionState::Connecting => Self::RTMP_STATE_CONNECTING,
            RtmpConnectionState::Connected => Self::RTMP_STATE_CONNECTED,
            RtmpConnectionState::MediaStreamCreated => Self::RTMP_STATE_MEDIA_STREAM_CREATED,
            RtmpConnectionState::PublishPending => Self::RTMP_STATE_PUBLISH_PENDING,
            RtmpConnectionState::Publishing => Self::RTMP_STATE_PUBLISHING,
            RtmpConnectionState::PlayPending => Self::RTMP_STATE_PLAY_PENDING,
            RtmpConnectionState::Playing => Self::RTMP_STATE_PLAYING,
            RtmpConnectionState::Disconnecting => Self::RTMP_STATE_DISCONNECTING,
        }
    }
}

/// 音声フォーマット
#[repr(C)]
#[derive(Clone, Copy)]
#[expect(non_camel_case_types)]
pub enum RtmpCAudioFormat {
    RTMP_AUDIO_FORMAT_ADPCM = 1,
    RTMP_AUDIO_FORMAT_MP3 = 2,
    RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN = 3,
    RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO = 4,
    RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO = 5,
    RTMP_AUDIO_FORMAT_NELLYMOSER = 6,
    RTMP_AUDIO_FORMAT_G711_ALAW = 7,
    RTMP_AUDIO_FORMAT_G711_MULAW = 8,
    RTMP_AUDIO_FORMAT_AAC = 10,
    RTMP_AUDIO_FORMAT_SPEEX = 11,
    RTMP_AUDIO_FORMAT_MP3_8KHZ = 14,
    RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND = 15,
}

impl From<AudioFormat> for RtmpCAudioFormat {
    fn from(format: AudioFormat) -> Self {
        match format {
            AudioFormat::Adpcm => Self::RTMP_AUDIO_FORMAT_ADPCM,
            AudioFormat::Mp3 => Self::RTMP_AUDIO_FORMAT_MP3,
            AudioFormat::LinearPcmLittleEndian => Self::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN,
            AudioFormat::Nellymoser16khzMono => Self::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO,
            AudioFormat::Nellymoser8KhzMono => Self::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO,
            AudioFormat::Nellymoser => Self::RTMP_AUDIO_FORMAT_NELLYMOSER,
            AudioFormat::G711AlawLogarithmicPcm => Self::RTMP_AUDIO_FORMAT_G711_ALAW,
            AudioFormat::G711MuLawLogarithmicPcm => Self::RTMP_AUDIO_FORMAT_G711_MULAW,
            AudioFormat::Aac => Self::RTMP_AUDIO_FORMAT_AAC,
            AudioFormat::Speex => Self::RTMP_AUDIO_FORMAT_SPEEX,
            AudioFormat::Mp3_8khz => Self::RTMP_AUDIO_FORMAT_MP3_8KHZ,
            AudioFormat::DeviceSpecificSound => Self::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND,
        }
    }
}

impl From<RtmpCAudioFormat> for AudioFormat {
    fn from(format: RtmpCAudioFormat) -> Self {
        match format {
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_ADPCM => Self::Adpcm,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_MP3 => Self::Mp3,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN => {
                Self::LinearPcmLittleEndian
            }
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO => Self::Nellymoser16khzMono,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO => Self::Nellymoser8KhzMono,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER => Self::Nellymoser,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_G711_ALAW => Self::G711AlawLogarithmicPcm,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_G711_MULAW => Self::G711MuLawLogarithmicPcm,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_AAC => Self::Aac,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_SPEEX => Self::Speex,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_MP3_8KHZ => Self::Mp3_8khz,
            RtmpCAudioFormat::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND => Self::DeviceSpecificSound,
        }
    }
}

/// 音声サンプリングレート
#[repr(C)]
#[derive(Clone, Copy)]
#[expect(non_camel_case_types)]
pub enum RtmpCAudioSampleRate {
    RTMP_AUDIO_SAMPLE_RATE_5KHZ = 0,
    RTMP_AUDIO_SAMPLE_RATE_11KHZ = 1,
    RTMP_AUDIO_SAMPLE_RATE_22KHZ = 2,
    RTMP_AUDIO_SAMPLE_RATE_44KHZ = 3,
}

impl From<AudioSampleRate> for RtmpCAudioSampleRate {
    fn from(rate: AudioSampleRate) -> Self {
        match rate {
            AudioSampleRate::Khz5 => Self::RTMP_AUDIO_SAMPLE_RATE_5KHZ,
            AudioSampleRate::Khz11 => Self::RTMP_AUDIO_SAMPLE_RATE_11KHZ,
            AudioSampleRate::Khz22 => Self::RTMP_AUDIO_SAMPLE_RATE_22KHZ,
            AudioSampleRate::Khz44 => Self::RTMP_AUDIO_SAMPLE_RATE_44KHZ,
        }
    }
}

impl From<RtmpCAudioSampleRate> for AudioSampleRate {
    fn from(rate: RtmpCAudioSampleRate) -> Self {
        match rate {
            RtmpCAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_5KHZ => Self::Khz5,
            RtmpCAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_11KHZ => Self::Khz11,
            RtmpCAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_22KHZ => Self::Khz22,
            RtmpCAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_44KHZ => Self::Khz44,
        }
    }
}

/// 映像コーデック
#[repr(C)]
#[derive(Clone, Copy)]
#[expect(non_camel_case_types)]
pub enum RtmpCVideoCodec {
    RTMP_VIDEO_CODEC_JPEG = 1,
    RTMP_VIDEO_CODEC_H263 = 2,
    RTMP_VIDEO_CODEC_SCREEN_VIDEO = 3,
    RTMP_VIDEO_CODEC_VP6 = 4,
    RTMP_VIDEO_CODEC_VP6_WITH_ALPHA = 5,
    RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 = 6,
    RTMP_VIDEO_CODEC_AVC = 7,
}

impl From<VideoCodec> for RtmpCVideoCodec {
    fn from(codec: VideoCodec) -> Self {
        match codec {
            VideoCodec::Jpeg => Self::RTMP_VIDEO_CODEC_JPEG,
            VideoCodec::H263 => Self::RTMP_VIDEO_CODEC_H263,
            VideoCodec::ScreenVideo => Self::RTMP_VIDEO_CODEC_SCREEN_VIDEO,
            VideoCodec::Vp6 => Self::RTMP_VIDEO_CODEC_VP6,
            VideoCodec::Vp6WithAlpha => Self::RTMP_VIDEO_CODEC_VP6_WITH_ALPHA,
            VideoCodec::ScreenVideoV2 => Self::RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2,
            VideoCodec::Avc => Self::RTMP_VIDEO_CODEC_AVC,
        }
    }
}

impl From<RtmpCVideoCodec> for VideoCodec {
    fn from(codec: RtmpCVideoCodec) -> Self {
        match codec {
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_JPEG => Self::Jpeg,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_H263 => Self::H263,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO => Self::ScreenVideo,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_VP6 => Self::Vp6,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_VP6_WITH_ALPHA => Self::Vp6WithAlpha,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 => Self::ScreenVideoV2,
            RtmpCVideoCodec::RTMP_VIDEO_CODEC_AVC => Self::Avc,
        }
    }
}

/// 映像フレームタイプ
#[repr(C)]
#[derive(Clone, Copy)]
#[expect(non_camel_case_types)]
pub enum RtmpCVideoFrameType {
    RTMP_VIDEO_FRAME_TYPE_KEY_FRAME = 1,
    RTMP_VIDEO_FRAME_TYPE_INTER_FRAME = 2,
    RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME = 3,
    RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME = 4,
    RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME = 5,
}

impl From<VideoFrameType> for RtmpCVideoFrameType {
    fn from(frame_type: VideoFrameType) -> Self {
        match frame_type {
            VideoFrameType::KeyFrame => Self::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME,
            VideoFrameType::InterFrame => Self::RTMP_VIDEO_FRAME_TYPE_INTER_FRAME,
            VideoFrameType::DisposableInterFrame => {
                Self::RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME
            }
            VideoFrameType::GeneratedKeyFrame => Self::RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME,
            VideoFrameType::VideoInfoOrCommandFrame => {
                Self::RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME
            }
        }
    }
}

impl From<RtmpCVideoFrameType> for VideoFrameType {
    fn from(frame_type: RtmpCVideoFrameType) -> Self {
        match frame_type {
            RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME => Self::KeyFrame,
            RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_INTER_FRAME => Self::InterFrame,
            RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME => {
                Self::DisposableInterFrame
            }
            RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME => {
                Self::GeneratedKeyFrame
            }
            RtmpCVideoFrameType::RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME => {
                Self::VideoInfoOrCommandFrame
            }
        }
    }
}

/// AVC パケットタイプ
#[repr(C)]
#[derive(Clone, Copy)]
#[expect(non_camel_case_types)]
pub enum RtmpCAvcPacketType {
    RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER = 0,
    RTMP_AVC_PACKET_TYPE_NAL_UNIT = 1,
    RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE = 2,
}

impl From<AvcPacketType> for RtmpCAvcPacketType {
    fn from(packet_type: AvcPacketType) -> Self {
        match packet_type {
            AvcPacketType::SequenceHeader => Self::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER,
            AvcPacketType::NalUnit => Self::RTMP_AVC_PACKET_TYPE_NAL_UNIT,
            AvcPacketType::EndOfSequence => Self::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE,
        }
    }
}

impl From<RtmpCAvcPacketType> for AvcPacketType {
    fn from(packet_type: RtmpCAvcPacketType) -> Self {
        match packet_type {
            RtmpCAvcPacketType::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER => Self::SequenceHeader,
            RtmpCAvcPacketType::RTMP_AVC_PACKET_TYPE_NAL_UNIT => Self::NalUnit,
            RtmpCAvcPacketType::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE => Self::EndOfSequence,
        }
    }
}

/// C 用の音声フレーム構造体
#[repr(C)]
pub struct RtmpCAudioFrame {
    pub timestamp_millis: u32,
    pub format: RtmpCAudioFormat,
    pub sample_rate: RtmpCAudioSampleRate,
    pub is_8bit_sample: bool,
    pub is_stereo: bool,
    pub is_aac_sequence_header: bool,
    pub data: *const u8,
    pub data_len: usize,
}

/// C 用の映像フレーム構造体
#[repr(C)]
pub struct RtmpCVideoFrame {
    pub timestamp_millis: u32,
    pub composition_timestamp_offset_millis: i32,
    pub frame_type: RtmpCVideoFrameType,
    pub codec: RtmpCVideoCodec,
    pub has_avc_packet_type: bool,
    pub avc_packet_type: RtmpCAvcPacketType,
    pub data: *const u8,
    pub data_len: usize,
}
