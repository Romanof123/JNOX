#include <jnox_pl031.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

/* UNIX seconds for 2032-02-29 23:59:58 UTC, then a two-second match on March 1.
 * Override at compile time or pass decimal epoch and timeout_ms after argv[0].
 * A manual RTC_TIMEOUT_MS=200 run exercises the bounded timeout (exit 24).
 */
#ifndef RTC_TEST_EPOCH
#define RTC_TEST_EPOCH 1961711998U
#endif
#ifndef RTC_TIMEOUT_MS
#define RTC_TIMEOUT_MS 5000U
#endif

volatile uint64_t jnox_rtc_driver_result[32];
#define R jnox_rtc_driver_result
#define NS_PER_S 1000000000ULL

static int monotonic(uint64_t *ns)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return -1;
    *ns = (uint64_t)t.tv_sec * NS_PER_S + (uint64_t)t.tv_nsec;
    return 0;
}

static int sleep_until(uint64_t ns)
{
    struct timespec t = {(time_t)(ns / NS_PER_S), (long)(ns % NS_PER_S)};
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &t, 0);
}

static int decimal(const char *s, uint32_t *out)
{
    uint32_t value = 0;
    if (!s || !*s) return -1;
    while (*s) {
        if (*s < '0' || *s > '9') return -1;
        unsigned digit = (unsigned)(*s++ - '0');
        if (value > (0xffffffffU - digit) / 10U) return -1;
        value = value * 10U + digit;
    }
    *out = value;
    return 0;
}

static int fail(unsigned code)
{
    R[2] = code;
    return (int)code;
}

int main(int argc, char **argv)
{
    struct jnox_pl031_stats stats = {0};
    uint32_t target = RTC_TEST_EPOCH, timeout = RTC_TIMEOUT_MS, seconds;
    unsigned enabled, pending;
    uint64_t now;

    R[0] = 0x504c303331443031ULL;
    R[1] = 1;
    if (argc < 1 || argc > 3 || (argc >= 2 && decimal(argv[1], &target)) ||
        (argc == 3 && decimal(argv[2], &timeout)) ||
        target > 0xfffffffcU || !timeout || timeout > 60000U) return fail(10);
    R[4] = target;
    R[6] = target + 2U;
    R[29] = timeout;
    if (monotonic(&now)) return fail(11);
    R[25] = now;
    if (jnox_pl031_bind()) return fail(12);
    jnox_pl031_stats(&stats);
    R[23] = stats.periphid;
    R[24] = stats.cellid;
    R[28] = stats.resources;
    R[1] = 2;
    if (jnox_pl031_read_time(&seconds)) return fail(13);
    R[3] = seconds;
    if (jnox_pl031_set_time(target) || jnox_pl031_read_time(&seconds)) return fail(14);
    R[5] = seconds;
    if (seconds < target || seconds > target + 1U) return fail(15);
    if (jnox_pl031_set_alarm(target + 2U, 1) ||
        jnox_pl031_read_alarm(&seconds, &enabled, &pending)) return fail(16);
    R[7] = seconds;
    R[8] = enabled;
    R[9] = pending;
    if (seconds != target + 2U || !enabled || pending) return fail(17);
    R[1] = 3;
    if (monotonic(&now)) return fail(18);
    uint64_t end = now + (uint64_t)timeout * 1000000ULL;
    for (;;) {
        if (monotonic(&now)) return fail(19);
        if (now >= end) return fail(24);
        if (jnox_pl031_read_alarm(&seconds, &enabled, &pending)) return fail(20);
        if (pending) {
            R[10] = pending;
            R[31] = seconds;
            if (!enabled || seconds != target + 2U) return fail(21);
            int handled = jnox_pl031_poll_irq();
            if (handled < 0) return fail(22);
            if (handled == 1) {
                R[11] = 1;
                if (monotonic(&now)) return fail(19);
                R[26] = now;
                break;
            }
        }
        uint64_t next = now + 10000000ULL;
        if (sleep_until(next < end ? next : end)) return fail(23);
    }
    R[1] = 4;
    if (jnox_pl031_read_alarm(&seconds, &enabled, &pending)) return fail(25);
    R[12] = pending;
    if (seconds != target + 2U || pending) return fail(26);
    if (jnox_pl031_read_time(&seconds)) return fail(27);
    R[13] = seconds;
    if (seconds < target + 2U) return fail(28);
    if (jnox_pl031_set_alarm(target + 2U, 0) ||
        jnox_pl031_read_alarm(&seconds, &enabled, &pending)) return fail(29);
    R[14] = enabled;
    if (enabled || pending) return fail(30);
    if (jnox_pl031_unbind()) return fail(31);
    jnox_pl031_stats(&stats);
    R[20] = stats.resources;
    if (stats.resources) return fail(32);
    R[1] = 5;
    if (jnox_pl031_bind() || jnox_pl031_read_time(&seconds)) return fail(33);
    R[21] = seconds;
    jnox_pl031_stats(&stats);
    R[30] = stats.resources;
    if (seconds < R[13] || !stats.resources || jnox_pl031_unbind()) return fail(34);
    jnox_pl031_stats(&stats);
    R[15] = stats.probes;
    R[16] = stats.removes;
    R[17] = stats.irq_calls;
    R[18] = stats.irq_events;
    R[19] = stats.irq_flags;
    R[22] = stats.resources;
    if (stats.probes != 2 || stats.removes != 2 || stats.resources || stats.irq_calls != 1 ||
        stats.irq_events != 1 || (stats.irq_flags & 0xa0U) != 0xa0U) return fail(35);
    if (monotonic(&now)) return fail(36);
    R[27] = now;
    R[1] = 100;
    (void)write(STDOUT_FILENO, "RTC DRIVER PASS\n", sizeof("RTC DRIVER PASS\n") - 1);
    return 0;
}
