#include <stdint.h>
#include <time.h>
#include <unistd.h>

volatile uint64_t jnox_queue_peer_result[16];
#define R jnox_queue_peer_result
#define NS_PER_S 1000000000ULL

static long net_call(unsigned long op, unsigned long arg)
{
    register unsigned long x0 __asm__("x0") = arg;
    register unsigned long x1 __asm__("x1") = 0;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static int run(void)
{
    R[3] = (uint64_t)net_call(161, 0);
    R[4] = (uint64_t)net_call(162, 1);
    if (R[3] != (uint64_t)-13L || R[4] != (uint64_t)-13L) return 30;
    uint64_t start = 0;
    for (unsigned i = 0; i < 6; ++i) {
        struct timespec t;
        uint64_t target = start + i * 20000000ULL;
        if (i) {
            t.tv_sec = (time_t)(target / NS_PER_S);
            t.tv_nsec = (long)(target % NS_PER_S);
            if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0)) return 31;
        }
        if (clock_gettime(CLOCK_MONOTONIC, &t)) return 32;
        uint64_t now = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
        if (!i) start = now;
        else if (now < target || now <= R[5 + i]) return 33;
        R[6 + i] = now;
        R[5] = i + 1;
    }
    return 0;
}

int main(void)
{
    R[0] = 0x5651504545303031ULL;
    R[1] = 1;
    int rc = run();
    if (rc) {
        R[2] = rc;
        return rc;
    }
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET QUEUE PEER PASS\n", sizeof("NET QUEUE PEER PASS\n") - 1);
    return 0;
}
