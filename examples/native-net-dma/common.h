#ifndef DMA_FIXTURE_H
#define DMA_FIXTURE_H
#include <jnox/ipc.h>
#include <time.h>
#include <jnox/notify.h>
#include <unistd.h>

#define LINE(message) write(STDOUT_FILENO, message "\n", sizeof(message))
enum { RX = 38, TX = 39, CLAIM = 76, RETIRE_WORD = 0x444d };
static unsigned char frame[1514];
static long raw(unsigned op, unsigned len)
{
    return jnox_ipc_trap(op, (uint64_t)frame, len, 0).status;
}
static long claim(void)
{
    return jnox_ipc_trap(CLAIM, 0, 0, 0).status;
}
static int pause_ms(long ms)
{
    struct timespec wake;
    if (clock_gettime(CLOCK_MONOTONIC, &wake)) return -1;
    wake.tv_nsec += ms % 1000 * 1000000L;
    wake.tv_sec += ms / 1000 + wake.tv_nsec / 1000000000L;
    wake.tv_nsec %= 1000000000L;
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &wake, 0);
}
#endif
