#ifndef DMA_FIXTURE_H
#define DMA_FIXTURE_H
#include <jnox/ipc.h>
#include <jnox/notify.h>
#include <jnox/raw_ethernet.h>
#include <time.h>

volatile uint64_t dma_phase;
enum { RETIRE_WORD = 0x444d };
static unsigned char frame[JNOX_ETHERNET_MAX_FRAME];

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
