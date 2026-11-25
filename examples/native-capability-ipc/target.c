#include <jnox/ipc.h>
#include <jnox/mailbox.h>
#include "protocol.h"

volatile uint64_t cap_target_magic = 0x43415054;
volatile uint64_t cap_target_words[8], cap_target_nwords;
volatile long cap_target_empty[3];
volatile uint64_t cap_target_nempty;

static int drain(unsigned first, unsigned count) {
    for (unsigned i = 0; i < count; ++i) {
        jnox_mailbox_result r = jnox_mailbox_receive(0);
        if (r.status != 0 || cap_target_nwords >= 8) return 0;
        cap_target_words[cap_target_nwords++] = r.word;
        if (r.word != SAMPLE(first + i)) return 0;
    }
    jnox_mailbox_result e = jnox_mailbox_receive(0);
    if (cap_target_nempty >= 3) return 0;
    cap_target_empty[cap_target_nempty++] = e.status;
    return e.status == -11;
}
int main(void) {
    for (;;) {
        jnox_ipc_request r = jnox_ipc_receive();
        if (r.status != 0) return 110;
        int ok;
        switch (r.word) {
        case DRAIN_FIRST: ok = drain(1, 3); break;
        case DRAIN_SECOND: ok = drain(4, 3); break;
        case DRAIN_BEFORE: ok = drain(7, 1); break;
        case DRAIN_ACTIVE: ok = drain(8, 1); break;
        case CHECK_EMPTY: ok = drain(0, 0); break;
        case DRAIN_FILL: ok = drain(9, 4); break;
        case DRAIN_RETRY: ok = drain(13, 1); break;
        case EXIT_NOW: return 73;
        default: return 111;
        }
        if (!ok || jnox_ipc_reply(r.token, ACK(r.word)) != 0) return 112;
    }
}
