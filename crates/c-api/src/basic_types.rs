use shiguredo_rtmp::{
    AudioFormat, AudioSampleRate, AvcPacketType, RtmpConnectionState as InnerConnectionState,
    VideoCodec, VideoFrameType,
};

/// RTMP 接続状態
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpConnectionState {
    RTMP_CONNECTION_STATE_HANDSHAKING = 0,
    RTMP_CONNECTION_STATE_CONNECTING,
    RTMP_CONNECTION_STATE_CONNECTED,
    RTMP_CONNECTION_STATE_MEDIA_STREAM_CREATED,
    RTMP_CONNECTION_STATE_PUBLISH_PENDING,
    RTMP_CONNECTION_STATE_PUBLISHING,
    RTMP_CONNECTION_STATE_PLAY_PENDING,
    RTMP_CONNECTION_STATE_PLAYING,
    RTMP_CONNECTION_STATE_DISCONNECTING,
}

impl RtmpConnectionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_CONNECTION_STATE_HANDSHAKING => "handshaking",
            Self::RTMP_CONNECTION_STATE_CONNECTING => "connecting",
            Self::RTMP_CONNECTION_STATE_CONNECTED => "connected",
            Self::RTMP_CONNECTION_STATE_MEDIA_STREAM_CREATED => "media_stream_created",
            Self::RTMP_CONNECTION_STATE_PUBLISH_PENDING => "publish_pending",
            Self::RTMP_CONNECTION_STATE_PUBLISHING => "publishing",
            Self::RTMP_CONNECTION_STATE_PLAY_PENDING => "play_pending",
            Self::RTMP_CONNECTION_STATE_PLAYING => "playing",
            Self::RTMP_CONNECTION_STATE_DISCONNECTING => "disconnecting",
        }
    }
}

impl From<InnerConnectionState> for RtmpConnectionState {
    fn from(state: InnerConnectionState) -> Self {
        match state {
            InnerConnectionState::Handshaking => Self::RTMP_CONNECTION_STATE_HANDSHAKING,
            InnerConnectionState::Connecting => Self::RTMP_CONNECTION_STATE_CONNECTING,
            InnerConnectionState::Connected => Self::RTMP_CONNECTION_STATE_CONNECTED,
            InnerConnectionState::MediaStreamCreated => {
                Self::RTMP_CONNECTION_STATE_MEDIA_STREAM_CREATED
            }
            InnerConnectionState::PublishPending => Self::RTMP_CONNECTION_STATE_PUBLISH_PENDING,
            InnerConnectionState::Publishing => Self::RTMP_CONNECTION_STATE_PUBLISHING,
            InnerConnectionState::PlayPending => Self::RTMP_CONNECTION_STATE_PLAY_PENDING,
            InnerConnectionState::Playing => Self::RTMP_CONNECTION_STATE_PLAYING,
            InnerConnectionState::Disconnecting => Self::RTMP_CONNECTION_STATE_DISCONNECTING,
        }
    }
}

/// RTMP イベント種別
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpConnectionEventKind {
    RTMP_CONNECTION_EVENT_KIND_NONE = 0,
    RTMP_CONNECTION_EVENT_KIND_PUBLISH_REQUESTED,
    RTMP_CONNECTION_EVENT_KIND_PLAY_REQUESTED,
    RTMP_CONNECTION_EVENT_KIND_AUDIO_RECEIVED,
    RTMP_CONNECTION_EVENT_KIND_VIDEO_RECEIVED,
    RTMP_CONNECTION_EVENT_KIND_STATE_CHANGED,
    RTMP_CONNECTION_EVENT_KIND_DISCONNECTED_BY_PEER,
    RTMP_CONNECTION_EVENT_KIND_COMMAND_IGNORED,
    RTMP_CONNECTION_EVENT_KIND_MESSAGE_IGNORED,
    RTMP_CONNECTION_EVENT_KIND_USER_CONTROL_EVENT_IGNORED,
}

impl RtmpConnectionEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_CONNECTION_EVENT_KIND_NONE => "none",
            Self::RTMP_CONNECTION_EVENT_KIND_PUBLISH_REQUESTED => "publish_requested",
            Self::RTMP_CONNECTION_EVENT_KIND_PLAY_REQUESTED => "play_requested",
            Self::RTMP_CONNECTION_EVENT_KIND_AUDIO_RECEIVED => "audio_received",
            Self::RTMP_CONNECTION_EVENT_KIND_VIDEO_RECEIVED => "video_received",
            Self::RTMP_CONNECTION_EVENT_KIND_STATE_CHANGED => "state_changed",
            Self::RTMP_CONNECTION_EVENT_KIND_DISCONNECTED_BY_PEER => "disconnected_by_peer",
            Self::RTMP_CONNECTION_EVENT_KIND_COMMAND_IGNORED => "command_ignored",
            Self::RTMP_CONNECTION_EVENT_KIND_MESSAGE_IGNORED => "message_ignored",
            Self::RTMP_CONNECTION_EVENT_KIND_USER_CONTROL_EVENT_IGNORED => {
                "user_control_event_ignored"
            }
        }
    }
}

/// 音声フォーマット
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpAudioFormat {
    RTMP_AUDIO_FORMAT_ADPCM = 1,
    RTMP_AUDIO_FORMAT_MP3 = 2,
    RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN = 3,
    RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO = 4,
    RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO = 5,
    RTMP_AUDIO_FORMAT_NELLYMOSER = 6,
    RTMP_AUDIO_FORMAT_G711_ALAW_LOGARITHMIC_PCM = 7,
    RTMP_AUDIO_FORMAT_G711_MULAW_LOGARITHMIC_PCM = 8,
    RTMP_AUDIO_FORMAT_AAC = 10,
    RTMP_AUDIO_FORMAT_SPEEX = 11,
    RTMP_AUDIO_FORMAT_MP3_8KHZ = 14,
    RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND = 15,
}

impl RtmpAudioFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_AUDIO_FORMAT_ADPCM => "adpcm",
            Self::RTMP_AUDIO_FORMAT_MP3 => "mp3",
            Self::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN => "linear_pcm_little_endian",
            Self::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO => "nellymoser_16khz_mono",
            Self::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO => "nellymoser_8khz_mono",
            Self::RTMP_AUDIO_FORMAT_NELLYMOSER => "nellymoser",
            Self::RTMP_AUDIO_FORMAT_G711_ALAW_LOGARITHMIC_PCM => "g711_alaw_logarithmic_pcm",
            Self::RTMP_AUDIO_FORMAT_G711_MULAW_LOGARITHMIC_PCM => "g711_mulaw_logarithmic_pcm",
            Self::RTMP_AUDIO_FORMAT_AAC => "aac",
            Self::RTMP_AUDIO_FORMAT_SPEEX => "speex",
            Self::RTMP_AUDIO_FORMAT_MP3_8KHZ => "mp3_8khz",
            Self::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND => "device_specific_sound",
        }
    }
}

impl From<AudioFormat> for RtmpAudioFormat {
    fn from(format: AudioFormat) -> Self {
        match format {
            AudioFormat::Adpcm => Self::RTMP_AUDIO_FORMAT_ADPCM,
            AudioFormat::Mp3 => Self::RTMP_AUDIO_FORMAT_MP3,
            AudioFormat::LinearPcmLittleEndian => Self::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN,
            AudioFormat::Nellymoser16khzMono => Self::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO,
            AudioFormat::Nellymoser8KhzMono => Self::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO,
            AudioFormat::Nellymoser => Self::RTMP_AUDIO_FORMAT_NELLYMOSER,
            AudioFormat::G711AlawLogarithmicPcm => {
                Self::RTMP_AUDIO_FORMAT_G711_ALAW_LOGARITHMIC_PCM
            }
            AudioFormat::G711MuLawLogarithmicPcm => {
                Self::RTMP_AUDIO_FORMAT_G711_MULAW_LOGARITHMIC_PCM
            }
            AudioFormat::Aac => Self::RTMP_AUDIO_FORMAT_AAC,
            AudioFormat::Speex => Self::RTMP_AUDIO_FORMAT_SPEEX,
            AudioFormat::Mp3_8khz => Self::RTMP_AUDIO_FORMAT_MP3_8KHZ,
            AudioFormat::DeviceSpecificSound => Self::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND,
        }
    }
}

impl From<RtmpAudioFormat> for AudioFormat {
    fn from(format: RtmpAudioFormat) -> Self {
        match format {
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_ADPCM => Self::Adpcm,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_MP3 => Self::Mp3,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_LINEAR_PCM_LITTLE_ENDIAN => {
                Self::LinearPcmLittleEndian
            }
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_16KHZ_MONO => Self::Nellymoser16khzMono,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER_8KHZ_MONO => Self::Nellymoser8KhzMono,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_NELLYMOSER => Self::Nellymoser,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_G711_ALAW_LOGARITHMIC_PCM => {
                Self::G711AlawLogarithmicPcm
            }
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_G711_MULAW_LOGARITHMIC_PCM => {
                Self::G711MuLawLogarithmicPcm
            }
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_AAC => Self::Aac,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_SPEEX => Self::Speex,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_MP3_8KHZ => Self::Mp3_8khz,
            RtmpAudioFormat::RTMP_AUDIO_FORMAT_DEVICE_SPECIFIC_SOUND => Self::DeviceSpecificSound,
        }
    }
}

/// 音声サンプルレート
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpAudioSampleRate {
    RTMP_AUDIO_SAMPLE_RATE_KHZ5 = 0,
    RTMP_AUDIO_SAMPLE_RATE_KHZ11 = 1,
    RTMP_AUDIO_SAMPLE_RATE_KHZ22 = 2,
    RTMP_AUDIO_SAMPLE_RATE_KHZ44 = 3,
}

impl RtmpAudioSampleRate {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_AUDIO_SAMPLE_RATE_KHZ5 => "5khz",
            Self::RTMP_AUDIO_SAMPLE_RATE_KHZ11 => "11khz",
            Self::RTMP_AUDIO_SAMPLE_RATE_KHZ22 => "22khz",
            Self::RTMP_AUDIO_SAMPLE_RATE_KHZ44 => "44khz",
        }
    }
}

impl From<AudioSampleRate> for RtmpAudioSampleRate {
    fn from(sample_rate: AudioSampleRate) -> Self {
        match sample_rate {
            AudioSampleRate::Khz5 => Self::RTMP_AUDIO_SAMPLE_RATE_KHZ5,
            AudioSampleRate::Khz11 => Self::RTMP_AUDIO_SAMPLE_RATE_KHZ11,
            AudioSampleRate::Khz22 => Self::RTMP_AUDIO_SAMPLE_RATE_KHZ22,
            AudioSampleRate::Khz44 => Self::RTMP_AUDIO_SAMPLE_RATE_KHZ44,
        }
    }
}

impl From<RtmpAudioSampleRate> for AudioSampleRate {
    fn from(sample_rate: RtmpAudioSampleRate) -> Self {
        match sample_rate {
            RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ5 => Self::Khz5,
            RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ11 => Self::Khz11,
            RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ22 => Self::Khz22,
            RtmpAudioSampleRate::RTMP_AUDIO_SAMPLE_RATE_KHZ44 => Self::Khz44,
        }
    }
}

/// 映像フレーム種別
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpVideoFrameType {
    RTMP_VIDEO_FRAME_TYPE_KEY_FRAME = 1,
    RTMP_VIDEO_FRAME_TYPE_INTER_FRAME = 2,
    RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME = 3,
    RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME = 4,
    RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME = 5,
}

impl RtmpVideoFrameType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME => "key_frame",
            Self::RTMP_VIDEO_FRAME_TYPE_INTER_FRAME => "inter_frame",
            Self::RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME => "disposable_inter_frame",
            Self::RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME => "generated_key_frame",
            Self::RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME => {
                "video_info_or_command_frame"
            }
        }
    }
}

impl From<VideoFrameType> for RtmpVideoFrameType {
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

impl From<RtmpVideoFrameType> for VideoFrameType {
    fn from(frame_type: RtmpVideoFrameType) -> Self {
        match frame_type {
            RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_KEY_FRAME => Self::KeyFrame,
            RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_INTER_FRAME => Self::InterFrame,
            RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME => {
                Self::DisposableInterFrame
            }
            RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME => {
                Self::GeneratedKeyFrame
            }
            RtmpVideoFrameType::RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME => {
                Self::VideoInfoOrCommandFrame
            }
        }
    }
}

/// 映像コーデック
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpVideoCodec {
    RTMP_VIDEO_CODEC_JPEG = 1,
    RTMP_VIDEO_CODEC_H263 = 2,
    RTMP_VIDEO_CODEC_SCREEN_VIDEO = 3,
    RTMP_VIDEO_CODEC_VP6 = 4,
    RTMP_VIDEO_CODEC_VP6_WITH_ALPHA = 5,
    RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 = 6,
    RTMP_VIDEO_CODEC_AVC = 7,
}

impl RtmpVideoCodec {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_VIDEO_CODEC_JPEG => "jpeg",
            Self::RTMP_VIDEO_CODEC_H263 => "h263",
            Self::RTMP_VIDEO_CODEC_SCREEN_VIDEO => "screen_video",
            Self::RTMP_VIDEO_CODEC_VP6 => "vp6",
            Self::RTMP_VIDEO_CODEC_VP6_WITH_ALPHA => "vp6_with_alpha",
            Self::RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 => "screen_video_v2",
            Self::RTMP_VIDEO_CODEC_AVC => "avc",
        }
    }
}

impl From<VideoCodec> for RtmpVideoCodec {
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

impl From<RtmpVideoCodec> for VideoCodec {
    fn from(codec: RtmpVideoCodec) -> Self {
        match codec {
            RtmpVideoCodec::RTMP_VIDEO_CODEC_JPEG => Self::Jpeg,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_H263 => Self::H263,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO => Self::ScreenVideo,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_VP6 => Self::Vp6,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_VP6_WITH_ALPHA => Self::Vp6WithAlpha,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 => Self::ScreenVideoV2,
            RtmpVideoCodec::RTMP_VIDEO_CODEC_AVC => Self::Avc,
        }
    }
}

/// AVC パケット種別
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(non_camel_case_types)]
pub enum RtmpAvcPacketType {
    RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER = 0,
    RTMP_AVC_PACKET_TYPE_NAL_UNIT = 1,
    RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE = 2,
}

impl RtmpAvcPacketType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER => "sequence_header",
            Self::RTMP_AVC_PACKET_TYPE_NAL_UNIT => "nal_unit",
            Self::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE => "end_of_sequence",
        }
    }
}

impl From<AvcPacketType> for RtmpAvcPacketType {
    fn from(packet_type: AvcPacketType) -> Self {
        match packet_type {
            AvcPacketType::SequenceHeader => Self::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER,
            AvcPacketType::NalUnit => Self::RTMP_AVC_PACKET_TYPE_NAL_UNIT,
            AvcPacketType::EndOfSequence => Self::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE,
        }
    }
}

impl From<RtmpAvcPacketType> for AvcPacketType {
    fn from(packet_type: RtmpAvcPacketType) -> Self {
        match packet_type {
            RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER => Self::SequenceHeader,
            RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_NAL_UNIT => Self::NalUnit,
            RtmpAvcPacketType::RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE => Self::EndOfSequence,
        }
    }
}
