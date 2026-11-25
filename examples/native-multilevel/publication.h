#ifndef JNOX_MULTILEVEL_PUBLICATION_H
#define JNOX_MULTILEVEL_PUBLICATION_H
#include <stdint.h>

static inline int publication_setting(int argc, char **argv, uint64_t *out)
{
    if (argc == 1) {
        *out = 0;
        return 1;
    }
    if (argc != 2)
        return 0;
    const char *text = argv[1];
    const char *prefix = "--publication-deadline-ms=";
    while (*prefix) {
        if (*text++ != *prefix++)
            return 0;
    }
    if (!*text)
        return 0;
    const uint64_t max_ms = UINT64_MAX / 1000000;
    uint64_t ms = 0;
    for (; *text; ++text) {
        if (*text < '0' || *text > '9')
            return 0;
        unsigned digit = (unsigned)(*text - '0');
        if (ms > (max_ms - digit) / 10)
            return 0;
        ms = ms * 10 + digit;
    }
    if (!ms)
        return 0;
    *out = ms * 1000000;
    return 1;
}

static inline int publication_target(uint64_t now, uint64_t *target)
{
    if (now > UINT64_MAX - 40000000)
        return 0;
    *target = now + 40000000;
    return 1;
}

static inline int publication_on_time(uint64_t target, uint64_t sample, uint64_t bound)
{
    return sample > target && sample - target <= bound;
}
#endif
