#include <errno.h>
#include <jnox/mailbox.h>
#include <time.h>

static const volatile uint32_t readings[] = {300, 512, 257, 1025};
volatile long mailbox_send_status[7];
volatile uint64_t mailbox_send_times[3]; /* before, absolute deadline, resumed */

static int monotonic_ns(uint64_t *ns) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) return 0;
    *ns = (uint64_t)t.tv_sec * 1000000000 + (uint64_t)t.tv_nsec;
    return 1;
}

int main(void) {
    for (unsigned i = 0; i < 4; ++i) {
        uint64_t word = ((uint64_t)(i + 1) << 32) | readings[i];
        mailbox_send_status[i] = jnox_mailbox_send(1, word, 0);
        if (mailbox_send_status[i] != 0) return 81;
    }
    mailbox_send_status[4] = jnox_mailbox_send(1, 0xeeee000000000005ULL, 0);
    if (mailbox_send_status[4] != -EAGAIN) return 82;

    uint64_t now;
    if (!monotonic_ns(&now) || now > UINT64_MAX - 100000000) return 83;
    mailbox_send_times[0] = now;
    uint64_t deadline = now + 100000000;
    mailbox_send_times[1] = deadline;
    mailbox_send_status[5] = jnox_mailbox_send(1, 0xdddd000000000005ULL, deadline);
    if (!monotonic_ns(&now)) return 84;
    mailbox_send_times[2] = now;
    if (mailbox_send_status[5] != -ETIMEDOUT || now < deadline) return 85;
    mailbox_send_status[6] = jnox_mailbox_send(1, 0x0000000500000058ULL, UINT64_MAX);
    return mailbox_send_status[6] == 0 ? 51 : 86;
}
