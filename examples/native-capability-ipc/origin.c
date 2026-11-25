#include <jnox/capability.h>
#include <jnox/ipc.h>
#include <jnox/mailbox.h>
#include "protocol.h"

/* Rows have operation ID, signed result in two's complement, and associated value. */
volatile uint64_t cap_origin_magic = 0x4341504f;
volatile uint64_t cap_origin_log[64][3], cap_origin_nlog;
volatile uint64_t cap_origin_counts[8][5], cap_origin_ncounts;
volatile long cap_origin_result = -1;

static int record(unsigned id, long status, long expected, uint64_t detail) {
    unsigned at = cap_origin_nlog;
    if (at >= 64) return 0;
    cap_origin_log[at][0] = id;
    cap_origin_log[at][1] = (uint64_t)status;
    cap_origin_log[at][2] = detail;
    cap_origin_nlog = at + 1;
    return status == expected;
}
static int counts(unsigned phase, unsigned handles, unsigned notices) {
    jnox_cap_counts c = {0};
    if (jnox_cap_observe(&c) != 0 || cap_origin_ncounts >= 8) return 0;
    unsigned row = cap_origin_ncounts++;
    cap_origin_counts[row][0] = phase;
    cap_origin_counts[row][1] = c.handles;
    cap_origin_counts[row][2] = c.notices;
    cap_origin_counts[row][3] = c.peak_handles;
    cap_origin_counts[row][4] = c.peak_notices;
    return c.handles == handles && c.notices == notices;
}
static int command(unsigned pid, unsigned cmd) {
    jnox_ipc_response r = jnox_ipc_call(pid, cmd, UINT64_MAX);
    return record(100 + cmd, r.status, 0, r.word) && r.word == ACK(cmd);
}
/* The addressee deliberately exits without replying. CALL returning EPIPE
 * establishes that this process has exited before the origin's next operation. */
static int ended(unsigned pid, unsigned cmd) {
    jnox_ipc_response r = jnox_ipc_call(pid, cmd, UINT64_MAX);
    return record(400 + cmd, r.status, -32, pid);
}
static int copy(jnox_cap_handle h, unsigned n, unsigned rights) {
    return record(20 + n, jnox_cap_copy(h, 1, rights, TAG(n)), 0, TAG(n));
}
static int send_sample(jnox_cap_handle h, unsigned n) {
    return record(60 + n, jnox_cap_send(h, SAMPLE(n), 0), 0, SAMPLE(n));
}
static int same(const char *a, const char *b) {
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}
static int transfer(jnox_cap_handle original) {
    if (!copy(original, 0, JNOX_CAP_SEND) || !copy(original, 1, JNOX_CAP_DELEGATE)) return 91;
    if (!record(30, jnox_cap_move(original, 1, BOTH, TAG(90)), -11, TAG(90)) ||
        !send_sample(original, 1) || !command(1, TAKE_FIRST)) return 92;
    if (!copy(original, 2, JNOX_CAP_SEND) || !copy(original, 3, JNOX_CAP_SEND) ||
        !command(1, TAKE_SECOND)) return 93;
    if (!record(31, jnox_cap_move(original, 1, BOTH, TAG(91)), -24, TAG(91)) ||
        !send_sample(original, 3) || !command(2, DRAIN_FIRST) || !command(1, FREE_FIRST)) return 94;

    if (!record(32, jnox_cap_move(original, 1, BOTH, TAG(4)), 0, TAG(4)) ||
        !record(33, jnox_cap_send(original, SAMPLE(99), 0), -116, original.raw) ||
        !record(34, jnox_cap_close(original), -116, original.raw)) return 95;
    jnox_cap_handle lookup = {0};
    if (!record(35, jnox_cap_initial(0, &lookup), -116, lookup.raw) ||
        !counts(2, 0, 0) || !command(1, TAKE_MOVED)) return 96;
    jnox_cap_notice back = {0};
    long rc = jnox_cap_receive(&back);
    if (!record(36, rc, 0, back.handle.raw) || back.sender != 1 || back.target != 2 ||
        back.rights != JNOX_CAP_SEND || back.tag != TAG(5) || back.handle.raw == original.raw) return 97;
    if (!send_sample(back.handle, 5) || !record(37, jnox_cap_receive(&back), -11, 0) ||
        !command(1, FINISH) || !command(2, DRAIN_SECOND)) return 98;
    if (!record(38, jnox_cap_copy(back.handle, 1, JNOX_CAP_SEND, TAG(92)), -13, back.handle.raw) ||
        !record(39, jnox_cap_close(back.handle), 0, back.handle.raw) || !counts(3, 0, 0)) return 99;
    return 71;
}
static int death_case(const char *scenario, jnox_cap_handle h) {
    if (same(scenario, "before")) {
        if (!ended(1, EXIT_NOW) ||
            !record(40, jnox_cap_move(h, 1, BOTH, TAG(6)), -32, h.raw) ||
            !send_sample(h, 7) || !command(2, DRAIN_BEFORE) ||
            !record(41, jnox_cap_close(h), 0, h.raw)) return 100;
    } else if (same(scenario, "pending") || same(scenario, "active")) {
        if (!record(42, jnox_cap_move(h, 1, JNOX_CAP_SEND, TAG(6)), 0, h.raw)) return 101;
        int active = same(scenario, "active");
        if (!ended(1, active ? ACTIVE_EXIT : PENDING_EXIT) ||
            !record(43, jnox_cap_send(h, SAMPLE(99), 0), -116, h.raw) ||
            !command(2, active ? DRAIN_ACTIVE : CHECK_EMPTY)) return 102;
    } else if (same(scenario, "target")) {
        if (!copy(h, 0, JNOX_CAP_SEND) || !command(1, HOLD_ACTIVE) ||
            !copy(h, 1, JNOX_CAP_SEND) || !ended(2, EXIT_NOW) ||
            !record(44, jnox_cap_send(h, SAMPLE(99), 0), -32, h.raw) ||
            !command(1, TARGET_GONE)) return 103;
    } else if (same(scenario, "mailbox")) {
        for (unsigned n = 9; n <= 12; ++n) if (!send_sample(h, n)) return 107;
        if (!record(45, jnox_cap_send(h, SAMPLE(13), 0), -11, SAMPLE(13)) ||
            !command(2, DRAIN_FILL) || !send_sample(h, 13) || !command(2, DRAIN_RETRY) ||
            !record(46, jnox_cap_close(h), 0, h.raw)) return 108;
    } else { return 104; }
    return counts(4, 0, 0) ? 71 : 105;
}
int main(int argc, char **argv) {
    int result = 90;
    jnox_cap_handle h = {0};
    long status = jnox_cap_initial(0, &h);
    if (argc == 2 && record(1, status, 0, h.raw) && counts(1, 1, 0) &&
        record(2, jnox_mailbox_send(2, SAMPLE(99), 0), -13, 2) &&
        record(3, jnox_cap_copy(h, 2, JNOX_CAP_SEND, TAG(94)), -13, 2)) {
        result = same(argv[1], "transfer") ? transfer(h) : death_case(argv[1], h);
    }
    /* Release companions even when an unexpected result ends a fixture early. */
    int receiver_ended = ended(1, EXIT_NOW);
    int target_ended = ended(2, EXIT_NOW);
    if (result == 71 && (!receiver_ended || !target_ended)) result = 106;
    cap_origin_result = result;
    return result;
}
