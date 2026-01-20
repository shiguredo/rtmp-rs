use crate::rtmp_timestamp::{RtmpTimestamp, RtmpTimestampDelta};

/// メディアフレーム（音声または映像）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaFrame {
    /// 音声フレーム
    Audio(AudioFrame),

    /// 映像フレーム
    Video(VideoFrame),
}

/// エンコードされた音声データを含む音声フレーム
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    /// このフレームのタイムスタンプ（ミリ秒単位）
    pub timestamp: RtmpTimestamp,

    /// 音声フォーマット（MP3、AAC など）
    pub format: AudioFormat,

    /// サンプリングレート（5.5kHz、11kHz、22kHz、44kHz のいずれか）
    pub sample_rate: AudioSampleRate,

    /// サンプルが 8 ビット（true）か 16 ビット（false）か
    pub is_8bit_sample: bool,

    /// 音声がステレオ（true）かモノラル（false）か
    pub is_stereo: bool,

    /// ペイロードデータが AAC シーケンスヘッダーであるかどうか
    pub is_aac_sequence_header: bool,

    /// エンコードされた音声データペイロード
    pub data: Vec<u8>,
}

/// エンコードされた映像データを含む映像フレーム
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// このフレームのタイムスタンプ（ミリ秒単位）
    pub timestamp: RtmpTimestamp,

    /// H.264/AVC の合成時間オフセット（デコード時刻と表示時刻の差分）
    pub composition_timestamp_offset: RtmpTimestampDelta,

    /// このフレームのタイプ（キーフレーム、インターフレーム など）
    pub frame_type: VideoFrameType,

    /// 使用されている映像コーデック（H.264、VP6 など）
    pub codec: VideoCodec,

    /// AVC パケットタイプ（H.264 の場合にペイロードデータが何を含むかを示す）
    pub avc_packet_type: Option<AvcPacketType>,

    /// エンコードされた映像データペイロード
    pub data: Vec<u8>,
}

impl VideoFrame {
    /// このフレームがキーフレームであるかどうかを返す
    pub fn is_keyframe(&self) -> bool {
        self.frame_type == VideoFrameType::KeyFrame
    }
}

/// AVC（H.264）パケットのタイプ
///
/// AVC 映像パケット内に含まれるデータの種類を示す
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AvcPacketType {
    /// デコーダー設定データを含む AVC シーケンスヘッダー
    SequenceHeader = 0,

    /// 映像データを含む 1 つ以上の NAL ユニット
    NalUnit = 1,

    /// AVC シーケンス終了マーカー
    EndOfSequence = 2,
}

/// 映像フレームのタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VideoFrameType {
    /// 独立してデコードできるキーフレーム（イントラフレーム）
    KeyFrame = 1,

    /// 前のフレームに依存するインターフレーム
    InterFrame = 2,

    /// 再生に影響を与えずにスキップできる破棄可能インターフレーム
    DisposableInterFrame = 3,

    /// 生成されたキーフレーム（サーバー使用予約）
    GeneratedKeyFrame = 4,

    /// 映像情報またはコマンドフレーム
    VideoInfoOrCommandFrame = 5,
}

/// エンコーディングに使用される映像コーデック
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VideoCodec {
    /// JPEG 画像コーデック
    Jpeg = 1,

    /// Sorenson H.263 コーデック
    H263 = 2,

    /// スクリーン映像コーデック
    ScreenVideo = 3,

    /// On2 VP6 コーデック
    Vp6 = 4,

    /// アルファチャネル付き On2 VP6 コーデック
    Vp6WithAlpha = 5,

    /// スクリーン映像バージョン 2 コーデック
    ScreenVideoV2 = 6,

    /// H.264/AVC コーデック
    Avc = 7,
}

/// エンコーディングに使用される音声フォーマット（コーデック）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioFormat {
    /// ADPCM コーデック
    Adpcm = 1,

    /// MP3 コーデック
    Mp3 = 2,

    /// リトルエンディアン リニア PCM コーデック
    LinearPcmLittleEndian = 3,

    /// Nellymoser コーデック（16kHz、モノラル）
    Nellymoser16khzMono = 4,

    /// Nellymoser コーデック（8kHz、モノラル）
    Nellymoser8KhzMono = 5,

    /// 様々なサンプリングレートの Nellymoser コーデック
    Nellymoser = 6,

    /// G.711 A-law 対数 PCM コーデック
    G711AlawLogarithmicPcm = 7,

    /// G.711 mu-law 対数 PCM コーデック
    G711MuLawLogarithmicPcm = 8,

    /// AAC コーデック
    Aac = 10,

    /// Speex コーデック
    Speex = 11,

    /// 8kHz の MP3 コーデック
    Mp3_8khz = 14,

    /// デバイス固有サウンドコーデック
    DeviceSpecificSound = 15,
}

/// 音声サンプリングレート
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioSampleRate {
    /// 5.5kHz サンプリングレート
    Khz5 = 0,

    /// 11kHz サンプリングレート
    Khz11 = 1,

    /// 22kHz サンプリングレート
    Khz22 = 2,

    /// 44kHz サンプリングレート
    Khz44 = 3,
}
