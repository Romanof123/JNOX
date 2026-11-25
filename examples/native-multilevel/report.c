#include "alarms.h"
#include "publication.h"
#include "../native-edf/clock.h"

volatile uint64_t request_ns[3];
volatile uint64_t observed_ns[3];
volatile uint32_t round_count[3];
volatile uint32_t cumulative_count[3];
volatile uint64_t publication_deadline_ns;
volatile uint32_t publication_marker[3];
volatile uint64_t publication_ns[3];
volatile uint32_t publication_verdict[3];

__attribute__((noinline))
void publication_release(unsigned round)
{
    __atomic_store_n(&publication_marker[round], round + 1, __ATOMIC_RELEASE);
}

static int checked_now(uint64_t *out)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) || now.tv_sec < 0 ||
        now.tv_nsec < 0 || now.tv_nsec >= 1000000000)
        return -1;
    uint64_t ns = (uint64_t)now.tv_nsec;
    if ((uint64_t)now.tv_sec > (UINT64_MAX - ns) / 1000000000)
        return -1;
    *out = (uint64_t)now.tv_sec * 1000000000 + ns;
    return 0;
}

int main(int argc, char **argv)
{
    uint64_t bound;
    if (!publication_setting(argc, argv, &bound))
        return 106;
    publication_deadline_ns = bound;
    int missed = 0;
    for (unsigned round = 0; round < 3; ++round) {
        uint64_t now, target;
        if (checked_now(&now) || !publication_target(now, &target))
            return 101;
        request_ns[round] = target;
        if (sleep_to(target) || checked_now(&now))
            return 102;
        observed_ns[round] = now;
        if (now < target)
            return 103;
        uint32_t count = count_alarms(round);
        publish_block(count);
        round_count[round] = count;
        cumulative_count[round] = private_result;
        if (count != 256 || private_result != (round + 1) * 256)
            return 104;
        if (bound) {
            publication_release(round);
            if (checked_now(&now))
                return 102;
            publication_ns[round] = now;
            if (now < observed_ns[round])
                return 103;
            int pass = publication_on_time(target, now, bound);
            publication_verdict[round] = pass ? 1 : 2;
            missed |= !pass;
        }
    }
    if (private_bytes != 12288 || private_blocks != 3)
        return 105;
    return missed ? 107 : 53;
}
