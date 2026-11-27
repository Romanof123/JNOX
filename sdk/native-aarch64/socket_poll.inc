#include <poll.h>
#include <time.h>

/* One byte per broker slot. Bit 7 registers the slot even for events == 0. */
int poll(struct pollfd *fds, nfds_t count, int timeout_ms) {
    if (count && !fds) return error(EFAULT);
    if (count > 2147483647UL) return error(EINVAL);
    if (!enter()) return -1;
    uint64_t mask = 0, deadline = UINT64_MAX;
    int invalid = 0;
    for (nfds_t i = 0; i < count; ++i) {
        int fd = fds[i].fd;
        fds[i].revents = 0;
        if (fd < 0) continue;
        if (fd <= 2 || (fd < FD_BASE && jnox_ipc_trap(49, fd, 1, 0).status >= 0))
            return finish(-ENOTSUP);
        if (backing_range(fd) && jnox_shm_size(fd) >= 0) return finish(-ENOTSUP);
        if (fd < FD_BASE || fd >= FD_BASE + FD_COUNT || !descriptors[fd-FD_BASE].kind) {
            fds[i].revents = POLLNVAL;
            ++invalid;
            continue;
        }
        unsigned slot = (unsigned)(fd-FD_BASE);
        if (descriptors[slot].family != AF_UNIX) return finish(-ENOTSUP);
        mask |= (uint64_t)(128 | (fds[i].events & (POLLIN | POLLOUT))) << (8*slot);
    }
    if (!mask && (invalid || !timeout_ms)) return finish(invalid);
    if (timeout_ms > 0 && !invalid) {
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now)) return finish(-errno);
        deadline = (uint64_t)now.tv_sec*1000000000ULL + now.tv_nsec
                 + (uint64_t)timeout_ms*1000000ULL;
        if (!mask) {
            struct timespec wake = {(long)(deadline/1000000000ULL), (long)(deadline%1000000000ULL)};
            int status = clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &wake, 0);
            return finish(status ? -status : 0);
        }
    }
    if (!configured) return finish(-ENOTCONN);
    uint64_t word = (0x17ULL << 56) | mask;
    if (invalid || !timeout_ms) word |= 1ULL << 32;
    jnox_ipc_response result = jnox_ipc_call(broker_pid, word, deadline);
    if (result.status == -ETIMEDOUT && timeout_ms > 0) return finish(invalid);
    if (result.status) return finish(result.status);
    if ((long)result.word < 0) return finish((long)result.word);
    int ready = invalid;
    for (nfds_t i = 0; i < count; ++i) {
        if (fds[i].fd < 0 || fds[i].revents == POLLNVAL) continue;
        unsigned slot = (unsigned)(fds[i].fd-FD_BASE);
        unsigned value = (result.word >> (slot*8)) & 255;
        fds[i].revents = value & ((fds[i].events & (POLLIN | POLLOUT)) | POLLHUP | POLLERR | POLLNVAL);
        if (fds[i].revents) ++ready;
    }
    return finish(ready);
}
