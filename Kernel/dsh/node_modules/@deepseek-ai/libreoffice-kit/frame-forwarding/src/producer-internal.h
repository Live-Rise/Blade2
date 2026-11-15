#pragma once
#include "dsh_frame_forwarding.h"

/** Private polling variant: distinguish a received credit from a timeout without
 * changing the public event kinds. Only one thread may poll a client.
 */
extern "C" int dsh_frame_forwarding_poll_activity(dsh_frame_forwarding_client* client, uint32_t timeout_ms,
    dsh_frame_forwarding_event* event, bool* received_packet);
