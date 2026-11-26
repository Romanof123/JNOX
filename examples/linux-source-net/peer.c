#include <stdint.h>
#include <time.h>
#include <unistd.h>

volatile uint64_t jnox_net_peer_result[16];
#define R jnox_net_peer_result
#define NS_PER_S 1000000000ULL

static long net_call(unsigned long number, unsigned long arg, unsigned long value)
{
    register unsigned long x0 __asm__("x0") = arg;
    register unsigned long x1 __asm__("x1") = value;
    register unsigned long x8 __asm__("x8") = number;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x1), "r"(x8) : "memory", "cc");
    return (long)x0;
}

static int fail(unsigned code)
{
    R[2] = code;
    return (int)code;
}

int main(void)
{
    R[0] = 0x564e455450303031ULL;
    R[1] = 1;
    R[3] = (uint64_t)net_call(156, 1, 0);
    R[4] = (uint64_t)net_call(157, 0, 0);
    R[5] = (uint64_t)net_call(158, 0, 1);
    R[6] = (uint64_t)net_call(159, 1U << 5, 1);
    R[7] = (uint64_t)net_call(160, 0, 0);
    for (unsigned i = 3; i <= 7; ++i)
        if (R[i] != (uint64_t)-13L) return fail(30 + i);
    R[1] = 2;
    uint64_t start = 0;
    for (unsigned i = 0; i < 6; ++i) {
        struct timespec t;
        uint64_t target = start + i * 20000000ULL;
        if (i) {
            t.tv_sec = (time_t)(target / NS_PER_S);
            t.tv_nsec = (long)(target % NS_PER_S);
            if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0)) return fail(40);
        }
        if (clock_gettime(CLOCK_MONOTONIC, &t)) return fail(41);
        uint64_t now = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
        if (!i) start = now;
        else if (now < target || now <= R[8 + i]) return fail(42);
        R[9 + i] = now;
        R[8] = i + 1;
    }
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET PEER PASS\n", sizeof("NET PEER PASS\n") - 1);
    return 0;
}
