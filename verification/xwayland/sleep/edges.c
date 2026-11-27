/* Drive the sleep adapter's error paths without claiming guest signal delivery. */
#include <errno.h>
#include <stddef.h>
#include <time.h>

unsigned int sleep(unsigned int seconds);
static struct timespec start, after, requested;
static int reads, waits, wait_error, clock_failure, reported_error;

void jnox_graphics_errno(int error) { reported_error = error; }

int jnox_poll_clock(int clock, struct timespec *ts)
{
    if (clock != CLOCK_MONOTONIC || ++reads == clock_failure) {
        jnox_graphics_errno(EFAULT);
        return -1;
    }
    *ts = reads == 1 ? start : after;
    return 0;
}

int jnox_poll_sleep(int clock, int flags, const struct timespec *ts, struct timespec *rest)
{
    ++waits;
    requested = *ts;
    if (clock != CLOCK_MONOTONIC || flags != TIMER_ABSTIME || rest != NULL) return EINVAL;
    return wait_error;
}

static int interrupted(long seconds, long ns, unsigned int left)
{
    reads = waits = reported_error = 0;
    after = (struct timespec){seconds, ns};
    wait_error = EINTR;
    return sleep(2) == left && reads == 2 && waits == 1 && reported_error == EINTR &&
           requested.tv_sec == 12 && requested.tv_nsec == 900000000L;
}

int main(void)
{
    if (sleep(0) || reads || waits || reported_error) return 1;
    clock_failure = 1;
    if (sleep(2) != 2 || waits || reported_error != EFAULT) return 2;
    clock_failure = reads = reported_error = 0;
    start = (struct timespec){__LONG_MAX__, 0};
    if (sleep(1) != 1 || waits || reported_error != EOVERFLOW) return 3;
    start = (struct timespec){10, 900000000L};
    if (!interrupted(11, 0, 2)) return 4;
    if (!interrupted(11, 900000000L, 1)) return 5;
    if (!interrupted(12, 899999999L, 1)) return 6;
    if (!interrupted(12, 900000000L, 0)) return 7;
    if (!interrupted(13, 0, 0)) return 8;
    clock_failure = 2;
    if (!interrupted(11, 900000000L, 2)) return 9;
    clock_failure = reads = waits = wait_error = reported_error = 0;
    if (sleep(~0U) || reads != 1 || waits != 1 || reported_error ||
        requested.tv_sec != (long)~0U + 10L || requested.tv_nsec != start.tv_nsec) return 10;
    return 0;
}
