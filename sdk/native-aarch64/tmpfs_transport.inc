/* Checked transaction shared by the Picolibc and freestanding clients.
 * Each runtime builds a single copy in the calling process. */
enum { OPEN = 1, UNLINK, STAT, CLOSE, FSTAT, READ, WRITE, SEEK, DUP, FLAGS,
       MKDIR, RENAME, LINK, CHMOD, FCHMOD, TMPFS_FSYNC };
enum { R = 1, W = 2, CREATE = 4, EXCL = 8, TRUNC = 16, APPEND = 32, CLOEXEC = 64 };
enum { MAX_PAYLOAD = 64, MAX_PATH_BYTES = 63, MAX_REQUEST = 2 * MAX_PATH_BYTES + 1, STAT_BYTES = 48 };
static unsigned char busy;

static long user_range(const void *p, size_t n, int writable)
{
    return jnox_ipc_trap(204, writable ? 2 : 1, (uintptr_t)p, n).status;
}
static long rpc(int broker, uint64_t word)
{
    jnox_ipc_response r = jnox_ipc_call(broker, word, UINT64_MAX);
    return r.status < 0 ? r.status : (long)r.word;
}
static long exchange(unsigned op, int fd, unsigned flags, uint32_t arg, const void *in, size_t n,
                     void *out, size_t capacity)
{
    if (n > MAX_REQUEST || capacity > MAX_PAYLOAD) return -22;
    long broker = jnox_ipc_trap(204, 0, 0, 0).status;
    if (broker < 0) return broker;
    if (n) { long rc = user_range(in, n, 0); if (rc < 0) return rc; }
    if (capacity) { long rc = user_range(out, capacity, 1); if (rc < 0) return rc; }
    if (__atomic_test_and_set(&busy, __ATOMIC_ACQUIRE)) return -16;
    const unsigned char *src = in;
    long rc = rpc(broker, (0x70ULL << 56) | n);
    for (size_t i = 0; rc == 0 && i < n; i += 6) {
        uint64_t chunk = (0x71ULL << 56) | ((uint64_t)i << 48);
        for (unsigned j = 0; j < 6 && i+j < n; ++j) chunk |= (uint64_t)src[i+j] << (8*j);
        rc = rpc(broker, chunk);
    }
    if (rc == 0) {
        uint64_t commit = (0x72ULL << 56) | ((uint64_t)op << 48) | ((uint64_t)(unsigned char)fd << 40)
                          | ((uint64_t)flags << 32) | arg;
        rc = rpc(broker, commit);
    }
    if (rc >= 0 && capacity != 0) {
        size_t count = op == READ ? (size_t)rc : STAT_BYTES;
        if (count > capacity) rc = -5;
        for (size_t i = 0; rc >= 0 && i < count; i += 6) {
            long chunk = rpc(broker, (0x73ULL << 56) | i);
            if (chunk < 0 || (uint64_t)chunk >> 48) { rc = chunk < 0 ? chunk : -5; break; }
            for (unsigned j = 0; j < 6 && i+j < count; ++j)
                ((unsigned char *)out)[i+j] = (unsigned char)((uint64_t)chunk >> (8*j));
        }
    }
    __atomic_clear(&busy, __ATOMIC_RELEASE);
    return rc;
}
