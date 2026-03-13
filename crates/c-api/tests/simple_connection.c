#include <assert.h>
#include <stddef.h>
#include <stdint.h>

#include "rtmp.h"

int main(void) {
  RtmpPublishClientConnection *connection = NULL;
  assert(rtmp_publish_client_connection_new("rtmp://localhost/live/test", &connection) ==
         RTMP_ERROR_OK);
  assert(connection != NULL);

  RtmpConnectionEvent *event = NULL;
  assert(rtmp_publish_client_connection_next_event(connection, &event) == RTMP_ERROR_OK);
  assert(event != NULL);
  assert(rtmp_connection_event_kind(event) == RTMP_CONNECTION_EVENT_KIND_STATE_CHANGED);

  RtmpConnectionState state = RTMP_CONNECTION_STATE_CONNECTED;
  assert(rtmp_connection_event_state(event, &state));
  assert(state == RTMP_CONNECTION_STATE_HANDSHAKING);
  rtmp_connection_event_free(event);

  const uint8_t *send_ptr = NULL;
  uintptr_t send_len = 0;
  assert(rtmp_publish_client_connection_get_send_buf(connection, &send_ptr, &send_len) ==
         RTMP_ERROR_OK);
  assert(send_len > 0);
  assert(send_ptr != NULL);
  assert(rtmp_publish_client_connection_advance_send_buf(connection, send_len) == RTMP_ERROR_OK);

  uint8_t payload[] = {1, 2, 3, 4};
  RtmpVideoFrame *frame = NULL;
  assert(rtmp_video_frame_new(0, 0, RTMP_VIDEO_FRAME_TYPE_KEY_FRAME, RTMP_VIDEO_CODEC_AVC, true,
                              RTMP_AVC_PACKET_TYPE_NAL_UNIT, payload, 4, &frame) ==
         RTMP_ERROR_OK);
  assert(frame != NULL);
  assert(rtmp_video_frame_data_len(frame) == 4);
  assert(rtmp_video_frame_data_ptr(frame) != NULL);
  rtmp_video_frame_free(frame);

  rtmp_publish_client_connection_free(connection);
  return 0;
}
