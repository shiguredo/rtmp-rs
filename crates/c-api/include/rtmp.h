#ifndef SHIGUREDO_RTMP_H
#define SHIGUREDO_RTMP_H

/* Generated with cbindgen:0.29.2 */

#include <stdbool.h>
#include <stdint.h>

/**
 * C API で返すエラーコード
 */
typedef enum RtmpError {
  RTMP_ERROR_OK = 0,
  RTMP_ERROR_INVALID_INPUT,
  RTMP_ERROR_INVALID_DATA,
  RTMP_ERROR_INVALID_STATE,
  RTMP_ERROR_UNSUPPORTED,
  RTMP_ERROR_NULL_POINTER,
  RTMP_ERROR_OTHER,
} RtmpError;

/**
 * 音声フォーマット
 */
typedef enum RtmpAudioFormat {
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
} RtmpAudioFormat;

/**
 * 音声サンプルレート
 */
typedef enum RtmpAudioSampleRate {
  RTMP_AUDIO_SAMPLE_RATE_KHZ5 = 0,
  RTMP_AUDIO_SAMPLE_RATE_KHZ11 = 1,
  RTMP_AUDIO_SAMPLE_RATE_KHZ22 = 2,
  RTMP_AUDIO_SAMPLE_RATE_KHZ44 = 3,
} RtmpAudioSampleRate;

/**
 * RTMP イベント種別
 */
typedef enum RtmpConnectionEventKind {
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
} RtmpConnectionEventKind;

/**
 * RTMP 接続状態
 */
typedef enum RtmpConnectionState {
  RTMP_CONNECTION_STATE_HANDSHAKING = 0,
  RTMP_CONNECTION_STATE_CONNECTING,
  RTMP_CONNECTION_STATE_CONNECTED,
  RTMP_CONNECTION_STATE_MEDIA_STREAM_CREATED,
  RTMP_CONNECTION_STATE_PUBLISH_PENDING,
  RTMP_CONNECTION_STATE_PUBLISHING,
  RTMP_CONNECTION_STATE_PLAY_PENDING,
  RTMP_CONNECTION_STATE_PLAYING,
  RTMP_CONNECTION_STATE_DISCONNECTING,
} RtmpConnectionState;

/**
 * 映像フレーム種別
 */
typedef enum RtmpVideoFrameType {
  RTMP_VIDEO_FRAME_TYPE_KEY_FRAME = 1,
  RTMP_VIDEO_FRAME_TYPE_INTER_FRAME = 2,
  RTMP_VIDEO_FRAME_TYPE_DISPOSABLE_INTER_FRAME = 3,
  RTMP_VIDEO_FRAME_TYPE_GENERATED_KEY_FRAME = 4,
  RTMP_VIDEO_FRAME_TYPE_VIDEO_INFO_OR_COMMAND_FRAME = 5,
} RtmpVideoFrameType;

/**
 * 映像コーデック
 */
typedef enum RtmpVideoCodec {
  RTMP_VIDEO_CODEC_JPEG = 1,
  RTMP_VIDEO_CODEC_H263 = 2,
  RTMP_VIDEO_CODEC_SCREEN_VIDEO = 3,
  RTMP_VIDEO_CODEC_VP6 = 4,
  RTMP_VIDEO_CODEC_VP6_WITH_ALPHA = 5,
  RTMP_VIDEO_CODEC_SCREEN_VIDEO_V2 = 6,
  RTMP_VIDEO_CODEC_AVC = 7,
} RtmpVideoCodec;

/**
 * AVC パケット種別
 */
typedef enum RtmpAvcPacketType {
  RTMP_AVC_PACKET_TYPE_SEQUENCE_HEADER = 0,
  RTMP_AVC_PACKET_TYPE_NAL_UNIT = 1,
  RTMP_AVC_PACKET_TYPE_END_OF_SEQUENCE = 2,
} RtmpAvcPacketType;

/**
 * 音声フレームの opaque ハンドル
 */
typedef struct RtmpAudioFrame RtmpAudioFrame;

/**
 * RTMP イベントの opaque ハンドル
 */
typedef struct RtmpConnectionEvent RtmpConnectionEvent;

/**
 * 再生用クライアント接続の opaque ハンドル
 */
typedef struct RtmpPlayClientConnection RtmpPlayClientConnection;

/**
 * 配信用クライアント接続の opaque ハンドル
 */
typedef struct RtmpPublishClientConnection RtmpPublishClientConnection;

/**
 * サーバー接続の opaque ハンドル
 */
typedef struct RtmpServerConnection RtmpServerConnection;

/**
 * 映像フレームの opaque ハンドル
 */
typedef struct RtmpVideoFrame RtmpVideoFrame;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * ライブラリのバージョンを取得する
 */
const char *rtmp_library_version(void);

/**
 * 音声フレームを作成する
 */
enum RtmpError rtmp_audio_frame_new(uint32_t timestamp_millis,
                                    enum RtmpAudioFormat format,
                                    enum RtmpAudioSampleRate sample_rate,
                                    bool is_8bit_sample,
                                    bool is_stereo,
                                    bool is_aac_sequence_header,
                                    const uint8_t *data,
                                    uintptr_t data_len,
                                    struct RtmpAudioFrame **out);

/**
 * 音声フレームを解放する
 */
void rtmp_audio_frame_free(struct RtmpAudioFrame *frame);

/**
 * 音声フレームのタイムスタンプを返す
 */
uint32_t rtmp_audio_frame_timestamp_millis(const struct RtmpAudioFrame *frame);

/**
 * 音声フレームのフォーマットを返す
 */
enum RtmpAudioFormat rtmp_audio_frame_format(const struct RtmpAudioFrame *frame);

/**
 * 音声フレームのサンプルレートを返す
 */
enum RtmpAudioSampleRate rtmp_audio_frame_sample_rate(const struct RtmpAudioFrame *frame);

/**
 * 8bit サンプルかを返す
 */
bool rtmp_audio_frame_is_8bit_sample(const struct RtmpAudioFrame *frame);

/**
 * ステレオかを返す
 */
bool rtmp_audio_frame_is_stereo(const struct RtmpAudioFrame *frame);

/**
 * AAC sequence header かを返す
 */
bool rtmp_audio_frame_is_aac_sequence_header(const struct RtmpAudioFrame *frame);

/**
 * ペイロード先頭ポインタを返す
 */
const uint8_t *rtmp_audio_frame_data_ptr(const struct RtmpAudioFrame *frame);

/**
 * ペイロード長を返す
 */
uintptr_t rtmp_audio_frame_data_len(const struct RtmpAudioFrame *frame);

/**
 * イベントを解放する
 */
void rtmp_connection_event_free(struct RtmpConnectionEvent *event);

/**
 * イベント種別を返す
 */
enum RtmpConnectionEventKind rtmp_connection_event_kind(const struct RtmpConnectionEvent *event);

/**
 * Publish / Play 系イベントの app を返す
 */
const char *rtmp_connection_event_app(const struct RtmpConnectionEvent *event);

/**
 * Publish / Play 系イベントの tc_url を返す
 */
const char *rtmp_connection_event_tc_url(const struct RtmpConnectionEvent *event);

/**
 * Publish / Play 系イベントの stream_name を返す
 */
const char *rtmp_connection_event_stream_name(const struct RtmpConnectionEvent *event);

/**
 * StateChanged イベントの状態を返す
 */
bool rtmp_connection_event_state(const struct RtmpConnectionEvent *event,
                                 enum RtmpConnectionState *out);

/**
 * AudioReceived イベントの音声フレームを返す
 */
const struct RtmpAudioFrame *rtmp_connection_event_audio_frame(const struct RtmpConnectionEvent *event);

/**
 * VideoReceived イベントの映像フレームを返す
 */
const struct RtmpVideoFrame *rtmp_connection_event_video_frame(const struct RtmpConnectionEvent *event);

/**
 * 切断理由を返す
 */
const char *rtmp_connection_event_reason(const struct RtmpConnectionEvent *event);

/**
 * ignored 系イベントの name を返す
 */
const char *rtmp_connection_event_name(const struct RtmpConnectionEvent *event);

/**
 * ignored 系イベントの detail を返す
 */
const char *rtmp_connection_event_detail(const struct RtmpConnectionEvent *event);

/**
 * 直近のエラーメッセージを取得する
 *
 * 返されるポインタは次に同一スレッドで C API が呼ばれるまで有効です。
 */
const char *rtmp_last_error_message(void);

/**
 * 直近のエラーメッセージをクリアする
 */
void rtmp_clear_last_error_message(void);

/**
 * 再生用クライアント接続を作成する
 */
enum RtmpError rtmp_play_client_connection_new(const char *url,
                                               struct RtmpPlayClientConnection **out);

/**
 * 再生用クライアント接続を解放する
 */
void rtmp_play_client_connection_free(struct RtmpPlayClientConnection *connection);

/**
 * 受信データを処理する
 */
enum RtmpError rtmp_play_client_connection_feed_recv_buf(struct RtmpPlayClientConnection *connection,
                                                         const uint8_t *buf,
                                                         uintptr_t buf_len);

/**
 * 送信バッファを取得する
 */
enum RtmpError rtmp_play_client_connection_get_send_buf(const struct RtmpPlayClientConnection *connection,
                                                        const uint8_t **out_ptr,
                                                        uintptr_t *out_len);

/**
 * 送信済みバイト数を進める
 */
enum RtmpError rtmp_play_client_connection_advance_send_buf(struct RtmpPlayClientConnection *connection,
                                                            uintptr_t n);

/**
 * 現在の状態を取得する
 */
enum RtmpError rtmp_play_client_connection_state(const struct RtmpPlayClientConnection *connection,
                                                 enum RtmpConnectionState *out);

/**
 * 次のイベントを取得する
 */
enum RtmpError rtmp_play_client_connection_next_event(struct RtmpPlayClientConnection *connection,
                                                      struct RtmpConnectionEvent **out);

/**
 * 配信用クライアント接続を作成する
 */
enum RtmpError rtmp_publish_client_connection_new(const char *url,
                                                  struct RtmpPublishClientConnection **out);

/**
 * 配信用クライアント接続を解放する
 */
void rtmp_publish_client_connection_free(struct RtmpPublishClientConnection *connection);

/**
 * 受信データを処理する
 */
enum RtmpError rtmp_publish_client_connection_feed_recv_buf(struct RtmpPublishClientConnection *connection,
                                                            const uint8_t *buf,
                                                            uintptr_t buf_len);

/**
 * 送信バッファを取得する
 */
enum RtmpError rtmp_publish_client_connection_get_send_buf(const struct RtmpPublishClientConnection *connection,
                                                           const uint8_t **out_ptr,
                                                           uintptr_t *out_len);

/**
 * 送信済みバイト数を進める
 */
enum RtmpError rtmp_publish_client_connection_advance_send_buf(struct RtmpPublishClientConnection *connection,
                                                               uintptr_t n);

/**
 * 現在の状態を取得する
 */
enum RtmpError rtmp_publish_client_connection_state(const struct RtmpPublishClientConnection *connection,
                                                    enum RtmpConnectionState *out);

/**
 * 音声フレームを送信する
 */
enum RtmpError rtmp_publish_client_connection_send_audio(struct RtmpPublishClientConnection *connection,
                                                         const struct RtmpAudioFrame *frame);

/**
 * 映像フレームを送信する
 */
enum RtmpError rtmp_publish_client_connection_send_video(struct RtmpPublishClientConnection *connection,
                                                         const struct RtmpVideoFrame *frame);

/**
 * 次のイベントを取得する
 */
enum RtmpError rtmp_publish_client_connection_next_event(struct RtmpPublishClientConnection *connection,
                                                         struct RtmpConnectionEvent **out);

/**
 * サーバー接続を作成する
 */
enum RtmpError rtmp_server_connection_new(struct RtmpServerConnection **out);

/**
 * サーバー接続を解放する
 */
void rtmp_server_connection_free(struct RtmpServerConnection *connection);

/**
 * 受信データを処理する
 */
enum RtmpError rtmp_server_connection_feed_recv_buf(struct RtmpServerConnection *connection,
                                                    const uint8_t *buf,
                                                    uintptr_t buf_len);

/**
 * 送信バッファを取得する
 */
enum RtmpError rtmp_server_connection_get_send_buf(const struct RtmpServerConnection *connection,
                                                   const uint8_t **out_ptr,
                                                   uintptr_t *out_len);

/**
 * 送信済みバイト数を進める
 */
enum RtmpError rtmp_server_connection_advance_send_buf(struct RtmpServerConnection *connection,
                                                       uintptr_t n);

/**
 * 現在の状態を取得する
 */
enum RtmpError rtmp_server_connection_state(const struct RtmpServerConnection *connection,
                                            enum RtmpConnectionState *out);

/**
 * 配信 / 再生要求を受理する
 */
enum RtmpError rtmp_server_connection_accept(struct RtmpServerConnection *connection);

/**
 * 配信 / 再生要求を拒否する
 */
enum RtmpError rtmp_server_connection_reject(struct RtmpServerConnection *connection,
                                             const char *reason);

/**
 * 音声フレームを送信する
 */
enum RtmpError rtmp_server_connection_send_audio(struct RtmpServerConnection *connection,
                                                 const struct RtmpAudioFrame *frame);

/**
 * 映像フレームを送信する
 */
enum RtmpError rtmp_server_connection_send_video(struct RtmpServerConnection *connection,
                                                 const struct RtmpVideoFrame *frame);

/**
 * 次のイベントを取得する
 */
enum RtmpError rtmp_server_connection_next_event(struct RtmpServerConnection *connection,
                                                 struct RtmpConnectionEvent **out);

/**
 * 映像フレームを作成する
 */
enum RtmpError rtmp_video_frame_new(uint32_t timestamp_millis,
                                    int32_t composition_timestamp_offset_millis,
                                    enum RtmpVideoFrameType frame_type,
                                    enum RtmpVideoCodec codec,
                                    bool has_avc_packet_type,
                                    enum RtmpAvcPacketType avc_packet_type,
                                    const uint8_t *data,
                                    uintptr_t data_len,
                                    struct RtmpVideoFrame **out);

/**
 * 映像フレームを解放する
 */
void rtmp_video_frame_free(struct RtmpVideoFrame *frame);

/**
 * 映像フレームのタイムスタンプを返す
 */
uint32_t rtmp_video_frame_timestamp_millis(const struct RtmpVideoFrame *frame);

/**
 * 合成時間オフセットを返す
 */
int32_t rtmp_video_frame_composition_timestamp_offset_millis(const struct RtmpVideoFrame *frame);

/**
 * フレーム種別を返す
 */
enum RtmpVideoFrameType rtmp_video_frame_frame_type(const struct RtmpVideoFrame *frame);

/**
 * コーデックを返す
 */
enum RtmpVideoCodec rtmp_video_frame_codec(const struct RtmpVideoFrame *frame);

/**
 * AVC パケット種別を返す
 */
bool rtmp_video_frame_avc_packet_type(const struct RtmpVideoFrame *frame,
                                      enum RtmpAvcPacketType *out);

/**
 * ペイロード先頭ポインタを返す
 */
const uint8_t *rtmp_video_frame_data_ptr(const struct RtmpVideoFrame *frame);

/**
 * ペイロード長を返す
 */
uintptr_t rtmp_video_frame_data_len(const struct RtmpVideoFrame *frame);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* SHIGUREDO_RTMP_H */
