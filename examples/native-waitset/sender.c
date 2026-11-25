#include <jnox/mailbox.h>
#include <jnox/notify.h>
#include <jnox/waitset.h>

volatile long waitset_send_status[5];
volatile long waitset_sender_foreign;
int main(void) {
    waitset_sender_foreign = jnox_waitset_attach(0, JNOX_WAITSET_MAILBOX, 0).status;
    if (waitset_sender_foreign >= 0) return 80;
    waitset_send_status[0] = jnox_mailbox_send(0, 0x000000010000012cULL, 0);
    waitset_send_status[1] = jnox_pulse_send(0, 17, 0x0000000200000200ULL);
    waitset_send_status[2] = jnox_event_set(0, 4);
    waitset_send_status[3] = jnox_event_set(0, 1);
    waitset_send_status[4] = jnox_event_set(0, 8);
    for (unsigned i = 0; i < 5; ++i)
        if (waitset_send_status[i] != 0) return 81 + i;
    return 72;
}
