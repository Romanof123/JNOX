#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <stddef.h>
#include <stdint.h>
#include <time.h>

int main(void)
{
    struct timespec resolution = {-1, -1};
    /* The scope of this fixture is QEMU virt's 62.5 MHz architectural counter. */
    if (clock_getres(CLOCK_MONOTONIC, &resolution) != 0 ||
        resolution.tv_sec != 0 || resolution.tv_nsec != 16) return 111;
    if (clock_getres(CLOCK_MONOTONIC, NULL) != 0) return 112;
    errno = 0;
    if (clock_getres((clockid_t)-1, &resolution) != -1 || errno != EINVAL ||
        resolution.tv_sec != 0 || resolution.tv_nsec != 16) return 113;
    errno = 0;
    if (clock_getres((clockid_t)-1, NULL) != -1 || errno != EINVAL) return 114;
    errno = 0;
    if (clock_getres(CLOCK_MONOTONIC, (struct timespec *)(uintptr_t)1) != -1 ||
        errno != EFAULT) return 115;
    return 76;
}
