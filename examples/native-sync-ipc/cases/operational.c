#include <errno.h>
#include <jnox/ipc.h>
#include <time.h>

/* Two 32-bit counts whose sum needs 33 bits. */
#define COUNTS 0xf000001190000007ULL
#define TOTAL  0x180000018ULL
#define DEADLINE_LEAD_NS 100000000ULL
#define SERVER_LAG_NS     50000000ULL

/* Each process has its own copy. A status of +1 marks an unused entry. */
volatile struct {
    long call_status[2], receive_status[2], reply_status[2];
    uint64_t call_result[2], received[2], token[2];
    uint64_t start_ns, end_ns, deadline_ns, after_wait_ns, checksum;
} ipc_case = {
    .call_status = {1, 1}, .receive_status = {1, 1}, .reply_status = {1, 1}
};

static long call(unsigned i, unsigned peer, uint64_t word, uint64_t deadline)
{
    jnox_ipc_response response = jnox_ipc_call(peer, word, deadline);
    ipc_case.call_status[i] = response.status;
    if (response.status == 0)
        ipc_case.call_result[i] = response.word;
    return response.status;
}

static long receive(unsigned i)
{
    jnox_ipc_request request = jnox_ipc_receive();
    ipc_case.receive_status[i] = request.status;
    if (request.status == 0) {
        ipc_case.received[i] = request.word;
        ipc_case.token[i] = request.token;
    }
    return request.status;
}

static long reply(unsigned i, uint64_t token, uint64_t word)
{
    long status = jnox_ipc_reply(token, word);
    ipc_case.reply_status[i] = status;
    return status;
}

static int stamp(volatile uint64_t *out)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t))
        return -1;
    *out = (uint64_t)t.tv_sec * 1000000000ULL + (uint64_t)t.tv_nsec;
    return 0;
}

static int sleep_until(uint64_t ns)
{
    struct timespec t = {(time_t)(ns / 1000000000ULL), (long)(ns % 1000000000ULL)};
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0);
}

static uint64_t sum_counts(uint64_t word)
{
    return (word >> 32) + (uint32_t)word;
}

static int high(void)
{
    if (call(0, 2, COUNTS, UINT64_MAX) || ipc_case.call_result[0] != TOTAL)
        return 81;
    return 51;
}

static int medium(void)
{
    volatile uint32_t samples[256];
    uint64_t sum = 0;
    for (unsigned i = 0; i < 256; ++i)
        samples[i] = i;
    for (unsigned batch = 0; batch < 4096; ++batch)
        for (unsigned i = 0; i < 256; ++i)
            sum += samples[i];
    ipc_case.checksum = sum;
    return sum == 133693440 ? 52 : 82;
}

static int low(void)
{
    if (receive(0) || ipc_case.received[0] != COUNTS)
        return 83;
    if (reply(0, ipc_case.token[0], sum_counts(ipc_case.received[0])))
        return 84;
    return 53;
}

static int timeout(void)
{
    uint64_t deadline = ipc_case.start_ns + DEADLINE_LEAD_NS;
    ipc_case.deadline_ns = deadline;
    if (call(0, 1, deadline, deadline) != -ETIMEDOUT)
        return 85;
    if (stamp(&ipc_case.after_wait_ns) || ipc_case.after_wait_ns < deadline)
        return 86;
    if (call(1, 1, COUNTS, UINT64_MAX) || ipc_case.call_result[1] != TOTAL)
        return 87;
    return 54;
}

static int late(void)
{
    if (receive(0))
        return 88;
    uint64_t deadline = ipc_case.received[0];
    ipc_case.deadline_ns = deadline;
    uint64_t before_sleep;
    if (stamp(&before_sleep) || before_sleep >= deadline ||
        deadline - before_sleep > DEADLINE_LEAD_NS)
        return 89;
    if (sleep_until(deadline + SERVER_LAG_NS) || stamp(&ipc_case.after_wait_ns) ||
        ipc_case.after_wait_ns < deadline + SERVER_LAG_NS)
        return 90;
    if (reply(0, ipc_case.token[0], 0) != -116) /* ESTALE */
        return 91;
    if (receive(1) || ipc_case.received[1] != COUNTS ||
        ipc_case.token[1] == ipc_case.token[0])
        return 92;
    if (reply(1, ipc_case.token[1], sum_counts(ipc_case.received[1])))
        return 93;
    return 55;
}

static int orphan(void)
{
    if (call(0, 1, COUNTS, UINT64_MAX) != -32) /* EPIPE, delivered call */
        return 94;
    if (call(1, 1, COUNTS, UINT64_MAX) != -32) /* already dead */
        return 95;
    return 56;
}

static int depart(void)
{
    return receive(0) == 0 && ipc_case.received[0] == COUNTS ? 57 : 96;
}

static int root(void)
{
    if (call(0, 1, COUNTS, UINT64_MAX) || ipc_case.call_result[0] != TOTAL)
        return 97;
    return 58;
}

static int relay(void)
{
    if (receive(0) || ipc_case.received[0] != COUNTS)
        return 98;
    uint64_t outer_token = ipc_case.token[0];
    if (call(0, 2, outer_token, UINT64_MAX) || ipc_case.call_result[0] != ~outer_token)
        return 99;
    if (reply(0, outer_token, sum_counts(ipc_case.received[0])))
        return 100;
    return 59;
}

static int leaf(void)
{
    if (receive(0))
        return 101;
    uint64_t outer_token = ipc_case.received[0];
    if (reply(0, outer_token, 0) != -EPERM)
        return 102;
    if (call(0, 0, COUNTS, UINT64_MAX) != -EDEADLK)
        return 103;
    if (reply(1, ipc_case.token[0], ~outer_token))
        return 104;
    return 60;
}

static int equal(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

int main(int argc, char **argv)
{
    static const struct { const char *name; int (*run)(void); } roles[] = {
        {"high", high}, {"medium", medium}, {"low", low},
        {"timeout", timeout}, {"late", late}, {"orphan", orphan}, {"depart", depart},
        {"root", root}, {"relay", relay}, {"leaf", leaf}
    };
    if (argc != 2 || stamp(&ipc_case.start_ns))
        return 105;
    for (unsigned i = 0; i < sizeof(roles) / sizeof(roles[0]); ++i) {
        if (equal(argv[1], roles[i].name)) {
            int result = roles[i].run();
            return stamp(&ipc_case.end_ns) == 0 ? result : 106;
        }
    }
    return 107;
}
