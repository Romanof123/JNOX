/* Run before the ordinary #444 producer in the same native package. */
#include <errno.h>
#include <jnox/shm.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <stdint.h>
#include <unistd.h>

volatile uint64_t jnox_xwl_shm_probe[8];
#define EXPECT(expr) do { if (!(expr)) { jnox_xwl_shm_probe[1] = __LINE__; \
    jnox_xwl_shm_probe[2] = errno; return 0; } } while (0)

static int shared_file_probe(void)
{
    int fd = memfd_create("xwayland-shared", MFD_CLOEXEC | MFD_ALLOW_SEALING);
    EXPECT(fd >= 0 && fcntl(fd, F_GETFD) == FD_CLOEXEC);
    EXPECT(fd >= 0 && jnox_shm_size(fd) == 0 && jnox_shm_get_seals(fd) == 0);
    EXPECT(jnox_shm_map(fd, JNOX_SHM_READ) == JNOX_SHM_MAP_FAILED && errno == EINVAL);
    EXPECT(fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK) == 0);
    EXPECT(ftruncate(fd, 4101) == 0);
    int ro = jnox_shm_dup_readonly(fd);
    EXPECT(ro >= 0 && fcntl(ro, F_GET_SEALS) == F_SEAL_SHRINK);
    EXPECT(fcntl(ro, F_GETFD) == 0 && fcntl(ro, F_SETFD, FD_CLOEXEC) == 0);
    EXPECT(fcntl(ro, F_GETFD) == FD_CLOEXEC);
    EXPECT(jnox_shm_resize(ro, 8192) == -1 && errno == EACCES);
    EXPECT(jnox_shm_add_seals(ro, JNOX_SHM_SEAL_GROW) == -1 && errno == EACCES);
    EXPECT(jnox_shm_resize(fd, 4000) == -1 && errno == EPERM);
    EXPECT(jnox_shm_resize(fd, JNOX_SHM_MAX_BYTES + 1u) == -1 && errno == EINVAL);
    EXPECT(jnox_shm_add_seals(fd, 8) == -1 && errno == ENOTSUP);
    EXPECT(mmap(0, 4100, PROT_READ, MAP_SHARED, fd, 0) == MAP_FAILED && errno == ENOTSUP);
    volatile unsigned char *rw = mmap(0, 4101, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    EXPECT(rw != JNOX_SHM_MAP_FAILED);
    rw[4100] = 0xb7;
    EXPECT(jnox_shm_resize(fd, 4200) == -1 && errno == EBUSY);
    EXPECT(munmap((void *)rw, 4100) == -1 && errno == ENOTSUP);
    EXPECT(munmap((void *)rw, 4101) == 0);
    EXPECT(jnox_shm_resize(fd, 4500) == 0 && jnox_shm_size(ro) == 4500);
    EXPECT(jnox_shm_add_seals(fd, JNOX_SHM_SEAL_GROW | JNOX_SHM_SEAL_SEAL) == 0);
    EXPECT(jnox_shm_resize(fd, 4501) == -1 && errno == EPERM);
    EXPECT(jnox_shm_add_seals(fd, 0) == -1 && errno == EPERM);
    const volatile unsigned char *r = jnox_shm_map(ro, JNOX_SHM_READ);
    EXPECT(r != JNOX_SHM_MAP_FAILED && jnox_shm_get_seals(ro) == 7);
    EXPECT(close(fd) == 0 && close(ro) == 0 && r[4100] == 0xb7);
    for (unsigned i = 4101; i < 4500; ++i) EXPECT(r[i] == 0);
    EXPECT(jnox_shm_unmap((void *)r) == 0);
    int next = jnox_shm_create_resizable();
    EXPECT(next >= 0 && jnox_shm_get_seals(next) == 0);
    EXPECT(jnox_shm_resize(next, 4200) == 0);
    rw = jnox_shm_map(next, JNOX_SHM_READ | JNOX_SHM_WRITE);
    EXPECT(rw != JNOX_SHM_MAP_FAILED && rw[4100] == 0);
    rw[4094] = 0x31;
    rw[4095] = 0xa9;
    rw[4199] = 0x47;
    EXPECT(jnox_shm_unmap((void *)rw) == 0);
    EXPECT(jnox_shm_resize(next, 4095) == 0 && jnox_shm_resize(next, 4200) == 0);
    r = jnox_shm_map(next, JNOX_SHM_READ);
    EXPECT(r != JNOX_SHM_MAP_FAILED && r[4094] == 0x31);
    for (unsigned i = 4095; i < 4200; ++i) EXPECT(r[i] == 0);
    EXPECT(jnox_shm_unmap((void *)r) == 0 && close(next) == 0);
    int fixed = jnox_shm_create(1);
    EXPECT(fixed >= 0 && jnox_shm_resize(fixed, 3) == -1 && errno == ENOTSUP);
    EXPECT(jnox_shm_get_seals(fixed) == -1 && errno == ENOTSUP && close(fixed) == 0);
    EXPECT(jnox_shm_create(0) == -1 && errno == EINVAL);
    jnox_xwl_shm_probe[0] = 1;
    return 1;
}
#undef EXPECT
