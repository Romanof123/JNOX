#include <stdint.h>
#include <time.h>
#include <unistd.h>

volatile uint64_t jnox_rtc_peer_result[16];
#define R jnox_rtc_peer_result
#define NS_PER_S 1000000000ULL

static long rtc_call(unsigned long number, unsigned long offset, unsigned long value)
{
    register unsigned long x0 __asm__("x0") = offset;
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
    R[0] = 0x504c303331503031ULL;
    R[1] = 1;
    /* Valid register and ownership operands. The status records distinguish
     * denied authority from invalid arguments, no pending IRQ or a busy claim. */
    R[3] = (uint64_t)rtc_call(152, 0, 0);
    R[4] = (uint64_t)rtc_call(153, 4, 0);
    R[5] = (uint64_t)rtc_call(154, 0, 0);
    R[6] = (uint64_t)rtc_call(155, 1, 0);
    for (unsigned i = 3; i <= 6; ++i)
        if (R[i] != (uint64_t)-13L) return fail(40 + i);
    R[1] = 2;
    uint64_t base = 0;
    for (unsigned i = 0; i < 8; ++i) {
        struct timespec t;
        uint64_t deadline = base + (uint64_t)i * 500000000ULL;
        if (i) {
            t.tv_sec = (time_t)(deadline / NS_PER_S);
            t.tv_nsec = (long)(deadline % NS_PER_S);
            if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0)) return fail(50);
        }
        if (clock_gettime(CLOCK_MONOTONIC, &t)) return fail(51);
        uint64_t now = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
        if (!i) base = now;
        else if (now < deadline || now <= R[7 + i]) return fail(52);
        R[8 + i] = now;
        R[7] = i + 1;
    }
    R[1] = 100;
    (void)write(STDOUT_FILENO, "RTC PEER PASS\n", sizeof("RTC PEER PASS\n") - 1);
    return 0;
}
