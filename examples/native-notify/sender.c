#include <errno.h>
#include <jnox/notify.h>

/* Channel code and a 64-bit sequence/reading pair for each sample. */
static const volatile uint32_t readings[4] = {300, 512, 257, 1025};
static const uint32_t channel[4] = {17, 17, 23, 23};
volatile long notify_send_status[8];

int main(void) {
    for (unsigned i = 0; i < 4; ++i) {
        uint64_t word = ((uint64_t)(i + 1) << 32) | readings[i];
        notify_send_status[i] = jnox_pulse_send(1, channel[i], word);
        if (notify_send_status[i] != 0) return 81;
    }
    notify_send_status[4] = jnox_pulse_send(1, 99, UINT64_MAX);
    if (notify_send_status[4] != -EAGAIN) return 82;
    const uint32_t masks[3] = {9, 1, 4};
    for (unsigned i = 0; i < 3; ++i) {
        notify_send_status[5 + i] = jnox_event_set(1, masks[i]);
        if (notify_send_status[5 + i] != 0) return 83;
    }
    return 61;
}
