#include <stdint.h>
#include <time.h>
#include <unistd.h>

volatile uint64_t jnox_packet_peer_result[17];
#define R jnox_packet_peer_result
#define NS_PER_S 1000000000ULL
static unsigned char packet_buffer[1526];

static long packet_call(unsigned long op, unsigned long address, unsigned long size)
{
    register unsigned long x0 __asm__("x0") = 1; /* First generation in this cold boot. */
    register unsigned long x1 __asm__("x1") = address;
    register unsigned long x2 __asm__("x2") = size;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static int run(void)
{
    for (unsigned op = 163; op <= 168; ++op) {
        unsigned long address = 0, size = 0;
        if (op == 164 || op == 167) {
            address = (uintptr_t)packet_buffer;
            size = op == 164 ? 72 : sizeof(packet_buffer);
        }
        long rc = packet_call(op, address, size);
        R[3 + op - 163] = (uint64_t)rc;
        if (rc != -13) return 31;
    }
    R[16] = (uint64_t)packet_call(169, 1, 0);
    if (R[16] != (uint64_t)-13L) return 31;
    uint64_t start = 0;
    for (unsigned i = 0; i < 6; ++i) {
        uint64_t target = start + i * 20000000ULL;
        struct timespec t;
        if (i) {
            t.tv_sec = (time_t)(target / NS_PER_S);
            t.tv_nsec = (long)(target % NS_PER_S);
            if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0)) return 32;
        }
        if (clock_gettime(CLOCK_MONOTONIC, &t)) return 33;
        uint64_t now = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
        if (!i) start = now;
        else if (now < target || now <= R[10 + i - 1]) return 34;
        R[10 + i] = now;
        R[9] = i + 1;
    }
    return 0;
}

int main(void)
{
    R[0] = 0x564e504b50454531ULL;
    R[1] = 1;
    int rc = run();
    R[2] = (unsigned)rc;
    if (rc) return rc;
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET PACKET PEER PASS\n", sizeof("NET PACKET PEER PASS\n") - 1);
    return 0;
}
