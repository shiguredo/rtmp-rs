use crate::Error;
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

impl AudioFrame {
    /// AAC 用のサンプルレート
    ///
    /// FLV の仕様で「AAC の場合は固定値を使用する」と規定されている
    /// （この値は無視されて、デコーダーはビットストリームから適切な値を取得する）
    pub const AAC_SAMPLE_RATE: AudioSampleRate = AudioSampleRate::Khz44;

    /// AAC 用のステレオフラグの値
    ///
    /// FLV の仕様で「AAC の場合は固定値を使用する」と規定されている
    /// （この値は無視されて、デコーダーはビットストリームから適切な値を取得する）
    pub const AAC_STEREO: bool = true;
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

/// AVC（H.264）デコーダ設定データ（AVCDecoderConfigurationRecord）を表す構造体
///
/// FLV の AVC シーケンスヘッダー内に含まれるデコーダ設定情報を表現します。
/// H.264 ビデオストリームをデコードするために必要な SPS（Sequence Parameter Set）
/// と PPS（Picture Parameter Set）を含みます。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvcSequenceHeader {
    /// 設定バージョン（通常は 1）
    pub configuration_version: u8,

    /// AVC プロファイルインジケーション
    pub avc_profile_indication: u8,

    /// プロファイル互換性フラグ
    pub profile_compatibility: u8,

    /// AVC レベルインジケーション
    pub avc_level_indication: u8,

    /// NAL ユニット長フィールドのサイズ - 1（通常は 3、つまり 4 バイト）
    pub length_size_minus_one: u8,

    /// Sequence Parameter Set（SPS）リスト
    pub sps_list: Vec<Vec<u8>>,

    /// Picture Parameter Set（PPS）リスト
    pub pps_list: Vec<Vec<u8>>,
}

impl AvcSequenceHeader {
    /// バイト列をパースして [`AvcSequenceHeader`] インスタンスを生成する
    ///
    /// 通常はこのバイト列は [`VideoFrame::avc_packet_type`] が [`AvcPacketType::SequenceHeader`] の場合に
    /// [`VideoFrame::data`] に格納されている値となる
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 7 {
            return Err(Error::invalid_data(
                "AVCDecoderConfigurationRecord too short",
            ));
        }

        let configuration_version = data[0];
        if configuration_version != 1 {
            return Err(Error::unsupported(format!(
                "unsupported configuration version: {}",
                configuration_version
            )));
        }

        let avc_profile_indication = data[1];
        let profile_compatibility = data[2];
        let avc_level_indication = data[3];
        let length_size_minus_one = data[4] & 0x03;

        let mut offset = 5;
        let mut sps_list = Vec::new();
        let mut pps_list = Vec::new();

        // SPS ユニット群をパース
        if offset >= data.len() {
            return Err(Error::invalid_data("incomplete SPS configuration"));
        }
        let num_sps = (data[offset] & 0x1F) as usize;
        offset += 1;

        for _ in 0..num_sps {
            if offset + 2 > data.len() {
                return Err(Error::invalid_data("incomplete SPS length field"));
            }
            let sps_length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;

            if offset + sps_length > data.len() {
                return Err(Error::invalid_data("incomplete SPS data"));
            }
            sps_list.push(data[offset..offset + sps_length].to_vec());
            offset += sps_length;
        }

        // PPS ユニット群をパース
        if offset >= data.len() {
            return Err(Error::invalid_data("incomplete PPS count field"));
        }
        let num_pps = data[offset] as usize;
        offset += 1;

        for _ in 0..num_pps {
            if offset + 2 > data.len() {
                return Err(Error::invalid_data("incomplete PPS length field"));
            }
            let pps_length = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;

            if offset + pps_length > data.len() {
                return Err(Error::invalid_data("incomplete PPS data"));
            }
            pps_list.push(data[offset..offset + pps_length].to_vec());
            offset += pps_length;
        }

        Ok(Self {
            configuration_version,
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size_minus_one,
            sps_list,
            pps_list,
        })
    }

    /// [`AvcSequenceHeader`] インスタンスを対応するバイト列に変換する
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();

        result.push(self.configuration_version);
        result.push(self.avc_profile_indication);
        result.push(self.profile_compatibility);
        result.push(self.avc_level_indication);
        result.push(0xFC | self.length_size_minus_one); // 上位 6 ビットは 1 で埋める

        // SPS 数と SPS リスト
        result.push(0xE0 | (self.sps_list.len() as u8)); // 上位 3 ビットは 1 で埋める
        for sps in &self.sps_list {
            result.extend_from_slice(&(sps.len() as u16).to_be_bytes());
            result.extend_from_slice(sps);
        }

        // PPS 数と PPS リスト
        result.push(self.pps_list.len() as u8);
        for pps in &self.pps_list {
            result.extend_from_slice(&(pps.len() as u16).to_be_bytes());
            result.extend_from_slice(pps);
        }

        result
    }
}
