#include <stdint.h>
#include <time.h>
#include <unistd.h>

extern int jnox_packet_context_enter_v1(unsigned int xmit_more);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern long jnox_net_setup_queues(unsigned mode, unsigned long long receipt[10]);
extern void jnox_net_forget_queues(void);

volatile uint64_t jnox_queue_driver_result[32];
#define R jnox_queue_driver_result
#define VERSION_1 (1ULL << 32)
#define MAC (1U << 5)
#define MTU (1U << 3)

static long net_call(unsigned long op, unsigned long arg, unsigned long val)
{
    register unsigned long x0 __asm__("x0") = arg;
    register unsigned long x1 __asm__("x1") = val;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static int allow_peer_to_run(void)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return -1;
    t.tv_nsec += 40000000L;
    if (t.tv_nsec >= 1000000000L) {
        ++t.tv_sec;
        t.tv_nsec -= 1000000000L;
    }
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0);
}

static int ownership_epoch(unsigned epoch)
{
    unsigned long long queue[10] = {0};
    long claim = net_call(156, 1, 0);
    if (!epoch) R[3] = (uint64_t)claim;
    if (claim) return 10;
    if (!epoch && allow_peer_to_run()) return 11;
    long low = net_call(157, 0, 0), high = net_call(157, 1, 0);
    if (!epoch) {
        R[4] = (uint64_t)low;
        R[5] = (uint64_t)high;
    }
    if (low < 0 || high < 0 || !(low & MAC) || !(high & 1)) return 12;
    unsigned long long features = VERSION_1 | ((unsigned)low & (MAC | MTU));
    unsigned mtu = 0, reads = 0, diagnostics = 0;
    long rc = jnox_net_validate(&features, 0, &mtu, &reads, &diagnostics);
    if (!epoch) {
        R[6] = features;
        R[7] = (uint64_t)rc;
        R[8] = mtu;
        R[9] = reads;
        R[10] = diagnostics;
    }
    if (rc || reads != !!(low & MTU) || diagnostics ||
        features != (VERSION_1 | MAC | ((low & MTU) && mtu >= 68 ? MTU : 0))) return 13;
    rc = net_call(159, (unsigned)features, features >> 32);
    if (!epoch) R[11] = (uint64_t)rc;
    if (rc) return 14;
    if (!epoch) {
        R[12] = (uint64_t)jnox_net_setup_queues(1, queue);
        R[13] = (uint64_t)jnox_net_setup_queues(2, queue);
        if (R[12] != (uint64_t)-22L || R[13] != (uint64_t)-22L) return 15;
    }
    rc = jnox_net_setup_queues(0, queue);
    if (!epoch) R[14] = (uint64_t)rc;
    if (rc) return 16;
    if (!epoch) {
        for (unsigned i = 0; i < 10; ++i) R[15 + i] = queue[i];
    } else {
        R[28] = queue[0];
        R[30] = queue[1];
        R[31] = queue[2];
    }
    if (!queue[0] || queue[7] != 3 || queue[8] || queue[9]) return 17;
    for (unsigned i = 0; i < 2; ++i) {
        if (queue[3 + i] != 32 || queue[5 + i] < 32 ||
            queue[1 + i] != ((queue[5 + i] << 32) | (32ULL << 16) | 1)) return 18;
    }
    if (!epoch) {
        R[25] = (uint64_t)net_call(160, 0, 0);
        if (R[25] != 11) return 19;
    } else {
        if (queue[0] <= R[15]) return 20;
        R[29] = (uint64_t)net_call(162, R[15], 0);
        if (R[29] != (uint64_t)-116L) return 21;
    }
    rc = net_call(156, 0, 0);
    if (!epoch) R[26] = (uint64_t)rc;
    if (rc) return 22;
    jnox_net_forget_queues();
    if (!epoch) {
        R[27] = (uint64_t)net_call(162, queue[0], 0);
        if (R[27] != (uint64_t)-116L) return 23;
    }
    return 0;
}

int main(void)
{
    R[0] = 0x5651445256303031ULL;
    for (unsigned epoch = 0; epoch < 2; ++epoch) {
        R[1] = epoch + 1;
        int rc = jnox_packet_context_enter_v1(0);
        if (rc) {
            R[2] = 24;
            return 24;
        }
        rc = ownership_epoch(epoch);
        jnox_packet_context_leave_v1();
        if (rc) {
            R[2] = rc;
            return rc;
        }
    }
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET QUEUE DRIVER PASS\n", sizeof("NET QUEUE DRIVER PASS\n") - 1);
    return 0;
}
