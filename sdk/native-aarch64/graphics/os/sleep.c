/* Use the native timespec and clock IDs behind Picolibc's sleep symbol. */
#include <errno.h>
#include <stddef.h>
#include <time.h>

int jnox_poll_clock(int clock, struct timespec *ts);
int jnox_poll_sleep(int clock, int flags, const struct timespec *ts, struct timespec *rest);
void jnox_graphics_errno(int error);

unsigned int sleep(unsigned int seconds)
{
    if (!seconds) return 0;

    struct timespec deadline;
    if (jnox_poll_clock(CLOCK_MONOTONIC, &deadline) < 0) return seconds;
    if (__builtin_add_overflow(deadline.tv_sec, seconds, &deadline.tv_sec)) {
        jnox_graphics_errno(EOVERFLOW);
        return seconds;
    }

    int error = jnox_poll_sleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, NULL);
    if (!error) return 0;

    /* An absolute wait has no kernel remainder. Round the unslept interval
     * up to seconds, bounded by the original request. */
    unsigned int left = seconds;
    struct timespec now;
    if (jnox_poll_clock(CLOCK_MONOTONIC, &now) == 0) {
        if (now.tv_sec > deadline.tv_sec) {
            left = 0;
        } else {
            unsigned long whole = (unsigned long)(deadline.tv_sec - now.tv_sec);
            if (whole < seconds) left = (unsigned int)whole + (now.tv_nsec < deadline.tv_nsec);
        }
    }
    jnox_graphics_errno(error);
    return left;
}
