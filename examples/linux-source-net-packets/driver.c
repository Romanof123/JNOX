#include <stdint.h>
#include <time.h>
#include <unistd.h>

extern int jnox_packet_context_enter_v1(unsigned int xmit_more);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern long jnox_net_setup_queues(unsigned mode, unsigned long long receipt[10]);
extern void jnox_net_forget_queues(void);
extern int jnox_original_tx_init(void);
extern int jnox_original_tx_frame(const unsigned char *frame, unsigned int len, unsigned int sequence);
extern int jnox_original_tx_complete(unsigned int sequence);

volatile uint64_t jnox_packet_driver_result[32];
#define R jnox_packet_driver_result
#define NS_PER_S 1000000000ULL
#define VERSION_1 (1ULL << 32)
#define MAC (1U << 5)
#define MTU (1U << 3)
enum { VNET_HEADER = 12, MAX_ETHERNET = 1514, FRAME_COUNT = 80 };
static const unsigned char host_mac[6] = {0x02, 0x4a, 0x4e, 0x4f, 0x58, 0x01};
static unsigned char device_mac[6], nonce[8];
static unsigned char rx_buffer[VNET_HEADER + MAX_ETHERNET];

static int bytes_equal(const unsigned char *left, const void *right, unsigned count)
{
    const unsigned char *bytes = right;
    for (unsigned i = 0; i < count; ++i) {
        if (left[i] != bytes[i]) return 0;
    }
    return 1;
}

static long net_call(unsigned long op, unsigned long arg, unsigned long value, unsigned long size)
{
    register unsigned long x0 __asm__("x0") = arg;
    register unsigned long x1 __asm__("x1") = value;
    register unsigned long x2 __asm__("x2") = size;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0), "+r"(x1), "+r"(x2) : "r"(x8) : "memory", "cc");
    return (long)x0;
}

static int monotonic_ns(uint64_t *value)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return -1;
    *value = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
    return 0;
}

static int poll_sleep(uint64_t deadline)
{
    uint64_t now;
    if (monotonic_ns(&now) || now >= deadline) return -1;
    uint64_t wake = now + 5000000ULL;
    if (wake > deadline) wake = deadline;
    struct timespec t = {(time_t)(wake / NS_PER_S), (long)(wake % NS_PER_S)};
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0);
}

static int configure(void)
{
    long low = net_call(157, 0, 0, 0), high = net_call(157, 1, 0, 0);
    R[22] = (uint64_t)low;
    R[23] = (uint64_t)high;
    if (low < 0 || high < 0 || !(low & MAC) || !(high & 1)) return 12;
    for (unsigned i = 0; i < sizeof(device_mac); ++i) {
        long byte = net_call(158, i, 1, 0);
        if (byte < 0 || byte > 255) return 13;
        device_mac[i] = (unsigned char)byte;
        R[5] = (R[5] << 8) | (unsigned)byte;
    }
    unsigned mtu = 0, reads = 0, diagnostics = 0;
    unsigned long long features = VERSION_1 | ((unsigned)low & (MAC | MTU));
    R[18] = (uint64_t)jnox_net_validate(&features, 0, &mtu, &reads, &diagnostics);
    R[4] = features;
    R[19] = mtu;
    R[20] = reads;
    R[21] = diagnostics;
    if (R[18] || reads != !!(low & MTU) || diagnostics ||
        features != (VERSION_1 | MAC | ((low & MTU) && mtu >= 68 ? MTU : 0))) return 14;
    if (net_call(159, (uint32_t)features, features >> 32, 0)) return 15;
    unsigned long long queue[10] = {0};
    if (jnox_net_setup_queues(0, queue)) return 16;
    R[3] = queue[0];
    R[24] = queue[1];
    R[25] = queue[2];
    if (!queue[0] || queue[7] != 3 || queue[8] || queue[9]) return 17;
    for (unsigned i = 0; i < 2; ++i) {
        if (queue[3 + i] != 32 || queue[5 + i] < 32 ||
            queue[1 + i] != ((queue[5 + i] << 32) | (32ULL << 16) | 1)) return 18;
    }
    R[26] = (uint64_t)net_call(160, 0, 0, 0);
    return R[26] == 11 ? 0 : 19;
}

static int acknowledge_interrupts(void)
{
    long status = net_call(168, R[3], 0, 0);
    if (status < 0 || (status & ~3L)) return -1;
    R[14] |= (unsigned long)status;
    return 0;
}

/* Only the private host's control frames can start or finish the transfer. */
static int receive_control(int final_ack)
{
    const char *magic = final_ack ? "JNOXACK!" : "JNOXCHAL";
    uint64_t start;
    if (monotonic_ns(&start)) return -1;
    for (;;) {
        long count = net_call(167, R[3], (uintptr_t)rx_buffer, sizeof(rx_buffer));
        if (count != -11 && (count < VNET_HEADER + 14 || count > (long)sizeof(rx_buffer))) return -1;
        const unsigned char *eth = rx_buffer + VNET_HEADER;
        if (count >= VNET_HEADER + 60 && bytes_equal(eth, device_mac, 6) &&
            bytes_equal(eth + 6, host_mac, 6) && eth[12] == 0x88 && eth[13] == 0xb5 &&
            bytes_equal(eth + 14, magic, 8)) {
            if (!final_ack) {
                __builtin_memcpy(nonce, eth + 22, sizeof(nonce));
                for (unsigned i = 0; i < sizeof(nonce); ++i) R[9] = (R[9] << 8) | nonce[i];
                R[8] = (unsigned long)count - VNET_HEADER;
                R[27] = (unsigned long)count;
                return acknowledge_interrupts();
            }
            if (bytes_equal(eth + 22, nonce, sizeof(nonce))) {
                R[29] = (unsigned long)count - VNET_HEADER;
                return acknowledge_interrupts();
            }
        }
        if (poll_sleep(start + 10 * NS_PER_S)) return -1;
    }
}

static int send_frame(unsigned int sequence)
{
    unsigned char eth[91];
    unsigned len = 60 + (sequence % 32);
    __builtin_memcpy(eth, host_mac, 6);
    __builtin_memcpy(eth + 6, device_mac, 6);
    eth[12] = 0x88;
    eth[13] = 0xb5;
    __builtin_memcpy(eth + 14, "JNOXDATA", 8);
    __builtin_memcpy(eth + 22, nonce, sizeof(nonce));
    for (unsigned i = 0; i < 4; ++i) eth[30 + i] = (unsigned char)(sequence >> (24 - 8 * i));
    for (unsigned i = 34; i < len; ++i) eth[i] = (unsigned char)(sequence + i);
    if (jnox_original_tx_frame(eth, len, sequence)) return 25;
    ++R[10];
    uint64_t start;
    if (monotonic_ns(&start)) return 26;
    for (;;) {
        int rc = jnox_original_tx_complete(sequence);
        if (!rc) break;
        if (rc != -11 || poll_sleep(start + NS_PER_S)) return 26;
    }
    ++R[11];
    R[12] += len;
    R[13] = sequence;
    return acknowledge_interrupts() ? 27 : 0;
}

static int transfer(void)
{
    R[6] = (uint64_t)net_call(163, R[3], 0, 0);
    R[7] = (uint64_t)net_call(160, 0, 0, 0);
    if (R[6] || R[7] != 15) return 20;
    R[28] = (uint64_t)jnox_original_tx_init();
    if (R[28]) return 21;
    R[1] = 3;
    (void)write(STDOUT_FILENO, "NET PACKET READY\n", sizeof("NET PACKET READY\n") - 1);
    if (receive_control(0)) return 22;
    R[1] = 4;
    for (unsigned i = 0; i < FRAME_COUNT; ++i) {
        int rc = send_frame(i);
        if (rc) return rc;
    }
    R[1] = 6;
    (void)write(STDOUT_FILENO, "NET PACKET WAIT ACK\n", sizeof("NET PACKET WAIT ACK\n") - 1);
    return receive_control(1) ? 28 : 0;
}

int main(void)
{
    R[0] = 0x564e504b44525631ULL;
    R[1] = 1;
    if (net_call(156, 1, 0, 0)) {
        R[2] = 10;
        return 10;
    }
    int rc = jnox_packet_context_enter_v1(0);
    if (rc) {
        rc = 11;
    } else {
        rc = configure();
        jnox_packet_context_leave_v1();
    }
    if (!rc) rc = transfer();
    R[15] = (uint64_t)net_call(156, 0, 0, 0);
    jnox_net_forget_queues();
    if (!rc && R[15]) rc = 29;
    if (!rc) {
        R[16] = (uint64_t)net_call(163, R[3], 0, 0);
        R[17] = (uint64_t)net_call(166, R[3], 0, 0);
        if (R[16] != (uint64_t)-116L || R[17] != (uint64_t)-116L) rc = 30;
    }
    R[2] = (unsigned)rc;
    if (rc) return rc;
    R[1] = 100;
    (void)write(STDOUT_FILENO, "NET PACKET DRIVER PASS\n", sizeof("NET PACKET DRIVER PASS\n") - 1);
    return 0;
}
