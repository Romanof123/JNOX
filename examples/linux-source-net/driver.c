#include <stdint.h>
#include <time.h>
#include <unistd.h>

extern long jnox_net_validate(unsigned long long *features, unsigned mode,
                              unsigned *mtu, unsigned *reads, unsigned *diagnostics);

volatile uint64_t jnox_net_driver_result[32];
#define R jnox_net_driver_result
#define MAC (1U << 5)
#define MTU (1U << 3)
#define VERSION_1 (1ULL << 32)

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
    R[0] = 0x564e455444303031ULL;
    R[1] = 1;
    R[3] = (uint64_t)net_call(156, 1, 0);
    if (R[3]) return fail(10);
    R[4] = (uint64_t)net_call(160, 0, 0);
    if (R[4] != 3) return fail(11);

    /* Sleep with the claim live so the independent FIFO peer can exercise
     * all five calls during this ownership epoch. */
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now)) return fail(12);
    now.tv_nsec += 40000000L;
    if (now.tv_nsec >= 1000000000L) {
        now.tv_nsec -= 1000000000L;
        ++now.tv_sec;
    }
    if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &now, 0)) return fail(13);

    R[1] = 2;
    long low = net_call(157, 0, 0);
    long high = net_call(157, 1, 0);
    R[5] = (uint64_t)low;
    R[6] = (uint64_t)high;
    if (low < 0 || high < 0 || !(low & MAC) || !(high & 1)) return fail(14);
    for (unsigned offset = 0; offset < 6; ++offset) {
        long byte = net_call(158, offset, 1);
        if (byte < 0 || byte > 255) return fail(15);
        R[7] |= (uint64_t)byte << (8 * offset);
    }
    if (low & MTU) {
        long mtu = net_call(158, 10, 2);
        if (mtu < 0 || mtu > 65535) return fail(16);
        R[8] = (uint64_t)mtu;
    }

    unsigned long long selected = ((uint32_t)low & (MAC | MTU)) | VERSION_1;
    unsigned long long features = selected;
    unsigned mtu = 0, reads = 0, diagnostics = 0;
    R[1] = 3;
    R[9] = selected;
    R[11] = (uint64_t)jnox_net_validate(&features, 0, &mtu, &reads, &diagnostics);
    R[10] = features;
    R[12] = mtu;
    R[13] = reads;
    R[14] = diagnostics;
    /* Original virtnet_validate clears the MTU feature if it is below ETH_MIN_MTU. */
    unsigned long long expected = selected;
    if ((selected & MTU) && R[8] < 68) expected &= ~(unsigned long long)MTU;
    if (R[11] || features != expected || diagnostics || reads != !!(selected & MTU) ||
        ((selected & MTU) && mtu != R[8])) return fail(17);
    for (unsigned mode = 1; mode <= 2; ++mode) {
        features = selected;
        mtu = reads = diagnostics = 0;
        unsigned i = mode == 1 ? 15 : 18;
        R[i] = (uint64_t)jnox_net_validate(&features, mode, &mtu, &reads, &diagnostics);
        R[i + 1] = reads;
        R[i + 2] = diagnostics;
        if (R[i] != (uint64_t)-22L || reads || diagnostics != 1) return fail(17 + mode);
    }

    R[1] = 4;
    /* CTRL_RX is outside the control-plane profile even if offered. */
    R[21] = (uint64_t)net_call(159, (uint32_t)R[10] | (1U << 18), R[10] >> 32);
    R[22] = (uint64_t)net_call(160, 0, 0);
    if (R[21] != (uint64_t)-22L || R[22] != R[4]) return fail(20);
    R[23] = (uint64_t)net_call(159, (uint32_t)R[10], R[10] >> 32);
    R[24] = (uint64_t)net_call(160, 0, 0);
    if (R[23] || R[24] != (3 | 8)) return fail(21);
    R[1] = 5;
    R[25] = (uint64_t)net_call(156, 0, 0);
    R[26] = (uint64_t)net_call(160, 0, 0);
    if (R[25] || R[26] != (uint64_t)-19L) return fail(22);
    R[27] = (uint64_t)net_call(156, 1, 0);
    R[28] = (uint64_t)net_call(160, 0, 0);
    if (R[27] || R[28] != R[4]) return fail(23);
    R[29] = (uint64_t)net_call(156, 0, 0);
    R[30] = (uint64_t)net_call(160, 0, 0);
    if (R[29] || R[30] != (uint64_t)-19L) return fail(24);
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET DRIVER PASS\n", sizeof("NET DRIVER PASS\n") - 1);
    return 0;
}
