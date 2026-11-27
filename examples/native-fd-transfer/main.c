/* Native SCM_RIGHTS fixture. The same ELF runs in three independent address spaces. */
#include <errno.h>
#include <jnox/display.h>
#include <jnox/ipc.h>
#include <jnox/shm.h>
#include <stdint.h>
#include <sys/un.h>
#include <time.h>

enum { WIDTH = JNOX_DISPLAY_WIDTH, HEIGHT = JNOX_DISPLAY_HEIGHT, BYTES = WIDTH * HEIGHT * 4 };
enum { ROLE, STAGE, SOURCE_FD, LOCAL_FD, MAP_VA, SECOND_VA, STALE_ERR, OTHER_ERR,
       HASH1, HASH2, SAMPLE1, SAMPLE2, SOURCE_SIZE, REUSED_SIZE, PREFIX, SUFFIX,
       TRUNC_FLAGS, TRUNC_LENGTH, PLAIN_BYTE, POOL_WITH_TWO, POOL_WITH_FOUR,
       POOL_ON_EXIT, RECLAIMED, UNMAP_ERR, DENIED_SEND, EXIT_EOF, CLOSED, PASSED_FD,
       REUSED_PASS_FD, CANCEL_STATUS, FAIL_LINE, FAIL_ERR };
volatile long long fd_transfer_witness[32] = { -1 };
#define W fd_transfer_witness
#define CHECK(test) do { if (!(test)) { W[FAIL_LINE] = __LINE__; W[FAIL_ERR] = errno; return 100; } } while (0)
static const struct sockaddr_un endpoint = {AF_UNIX, "/fd-transfer"};
union control { struct cmsghdr align; unsigned char bytes[CMSG_SPACE(sizeof(int))]; };

static int delay(void)
{
    struct timespec until;
    if (clock_gettime(CLOCK_MONOTONIC, &until) < 0) return 0;
    until.tv_nsec += 10000000;
    if (until.tv_nsec >= 1000000000) { ++until.tv_sec; until.tv_nsec -= 1000000000; }
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &until, 0) == 0;
}

static int wait_key(int key)
{
    for (;;) {
        long b = jnox_display_poll_key();
        if (b == key) return 1;
        if (b != -EAGAIN || !delay()) return 0;
    }
}

static int put_byte(int fd, unsigned char b)
{
    return send(fd, &b, 1, 0) == 1;
}

static int get_byte(int fd)
{
    unsigned char b;
    return recv(fd, &b, 1, 0) == 1 ? b : -1;
}

/* Ordinary recvmsg has no deadline argument. Use the broker's public socket
 * transaction format here to give only COMMIT an absolute IPC deadline. */
static int cancel_empty_rights_receive(int fd)
{
    if (fd < 64 || fd > 67) return 0;
    uint64_t begin = (0x11ULL << 56) | (16ULL << 48) | ((uint64_t)(fd - 64) << 40) |
                     (1ULL << 32) | (1ULL << 9);
    jnox_ipc_response started = jnox_ipc_call(3, begin, UINT64_MAX);
    if (started.status != 0 || started.word != 0) return 0;
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return 0;
    uint64_t deadline = ((uint64_t)now.tv_sec + 2) * 1000000000ULL + (uint64_t)now.tv_nsec;
    W[STAGE] = 9;
    jnox_ipc_response commit = jnox_ipc_call(3, 0x13ULL << 56, deadline);
    W[CANCEL_STATUS] = commit.status;
    return commit.status == -ETIMEDOUT;
}

static int connection(unsigned char id)
{
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) return -1;
    while (connect(fd, (const struct sockaddr *)&endpoint, sizeof(endpoint)) < 0) {
        if ((errno != ENOENT && errno != ECONNREFUSED && errno != EAGAIN) || !delay()) return -1;
    }
    return put_byte(fd, id) ? fd : -1;
}

static ssize_t send_right(int socket_fd, int shared_fd, const char *bytes, unsigned size)
{
    union control buf = {0};
    struct iovec vec[2] = {{(void *)bytes, 1}, {(void *)(bytes + 1), size - 1}};
    struct msghdr msg = {0};
    msg.msg_iov = vec;
    msg.msg_iovlen = size > 1 ? 2 : 1;
    msg.msg_control = buf.bytes;
    msg.msg_controllen = sizeof(buf.bytes);
    struct cmsghdr *cm = CMSG_FIRSTHDR(&msg);
    cm->cmsg_len = CMSG_LEN(sizeof(int));
    cm->cmsg_level = SOL_SOCKET;
    cm->cmsg_type = SCM_RIGHTS;
    __builtin_memcpy(CMSG_DATA(cm), &shared_fd, sizeof(shared_fd));
    return sendmsg(socket_fd, &msg, 0);
}

/* Return the payload length. A control buffer of one byte deliberately truncates rights. */
static ssize_t receive(int fd, char *bytes, unsigned size, unsigned capacity,
                       int *shared_fd, int *flags, size_t *used)
{
    union control buf = {0};
    struct iovec vec = {bytes, size};
    struct msghdr msg = {0};
    msg.msg_iov = &vec;
    msg.msg_iovlen = 1;
    msg.msg_control = buf.bytes;
    msg.msg_controllen = capacity;
    ssize_t n = recvmsg(fd, &msg, 0);
    if (n < 0) return -1;
    *shared_fd = -1;
    *flags = msg.msg_flags;
    *used = msg.msg_controllen;
    if (!msg.msg_controllen) return n;
    struct cmsghdr *cm = CMSG_FIRSTHDR(&msg);
    if (!cm || msg.msg_controllen > capacity || cm->cmsg_len != CMSG_LEN(sizeof(int)) ||
        cm->cmsg_level != SOL_SOCKET || cm->cmsg_type != SCM_RIGHTS) return -1;
    __builtin_memcpy(shared_fd, CMSG_DATA(cm), sizeof(*shared_fd));
    return n;
}

static uint32_t pattern(unsigned x, unsigned y, unsigned second)
{
    unsigned r = (x + (second ? 160 : 32)) & 255;
    unsigned g = (y + (second ? 112 : 64)) & 255;
    unsigned b = (((x >> 4) ^ (y >> 4) ^ second) & 1) ? 0xd8 : 0x38;
    return (r << 16) | (g << 8) | b;
}

static void fill(volatile uint32_t *map, unsigned second)
{
    uint32_t hash = 2166136261u;
    for (unsigned y = 0; y < HEIGHT; ++y)
        for (unsigned x = 0; x < WIDTH; ++x) {
            uint32_t pixel = pattern(x, y, second);
            map[y * WIDTH + x] = pixel;
            hash = (hash ^ pixel) * 16777619u;
        }
    W[HASH1 + second] = hash;
    W[SAMPLE1 + second] = map[0];
}

static int present(const volatile uint32_t *map, unsigned second)
{
    uint32_t row[WIDTH], hash = 2166136261u;
    for (unsigned y = 0; y < HEIGHT; ++y) {
        for (unsigned x = 0; x < WIDTH; ++x) {
            uint32_t pixel = map[y * WIDTH + x];
            if (pixel != pattern(x, y, second)) return 0;
            row[x] = pixel;
            hash = (hash ^ pixel) * 16777619u;
        }
        if (jnox_display_write_row(y, row) < 0) return 0;
    }
    W[HASH1 + second] = hash;
    W[SAMPLE1 + second] = map[0];
    return 1;
}

static int producer(void)
{
    int data = connection('D'), gate = connection('P');
    CHECK(data >= 0 && gate >= 0);
    CHECK(get_byte(gate) == 's');
    int fd = jnox_shm_create(BYTES);
    CHECK(fd >= 0);
    W[SOURCE_FD] = fd;
    W[SOURCE_SIZE] = jnox_shm_size(fd);
    volatile uint32_t *map = jnox_shm_map(fd, JNOX_SHM_READ | JNOX_SHM_WRITE);
    CHECK(map != JNOX_SHM_MAP_FAILED && W[SOURCE_SIZE] == BYTES);
    W[MAP_VA] = (uintptr_t)map;
    fill(map, 0);
    int readonly = jnox_shm_dup_readonly(fd);
    CHECK(readonly >= 0 && readonly != fd);
    W[PASSED_FD] = readonly;
    CHECK(send(data, "pr", 2, 0) == 2);
    CHECK(send_right(data, readonly, "FD1", 3) == 3);
    CHECK(jnox_shm_close(readonly) == 0 && jnox_shm_close(fd) == 0);
    CHECK(jnox_shm_size(readonly) == -1 && errno == EBADF);
    CHECK(jnox_shm_map(fd, JNOX_SHM_READ) == JNOX_SHM_MAP_FAILED);
    W[STALE_ERR] = errno;
    CHECK(W[STALE_ERR] == EBADF);
    int decoy = jnox_shm_create(sizeof(uint32_t));
    CHECK(decoy == fd);
    int decoy_ro = jnox_shm_dup_readonly(decoy);
    CHECK(decoy_ro == readonly && jnox_shm_size(decoy_ro) == 4);
    W[REUSED_PASS_FD] = decoy_ro;
    W[LOCAL_FD] = decoy;
    W[REUSED_SIZE] = jnox_shm_size(decoy);
    volatile uint32_t *other = jnox_shm_map(decoy, JNOX_SHM_READ | JNOX_SHM_WRITE);
    CHECK(other != JNOX_SHM_MAP_FAILED && other != map && W[REUSED_SIZE] == 4 && *other == 0);
    W[SECOND_VA] = (uintptr_t)other;
    *other = 0xa1b2c3d4;
    CHECK(put_byte(data, 'z') && put_byte(gate, 'Q') && put_byte(gate, fd) && put_byte(gate, readonly));
    CHECK(get_byte(gate) == 'H');
    W[STAGE] = 1;
    CHECK(put_byte(gate, 'h') && get_byte(gate) == 'u');
    fill(map, 1);
    CHECK(*other == 0xa1b2c3d4);
    W[STAGE] = 2;
    CHECK(put_byte(gate, 'U') && get_byte(gate) == 'q');
    CHECK(jnox_shm_unmap((void *)map) == 0 && jnox_shm_unmap((void *)other) == 0);
    CHECK(jnox_shm_close(decoy_ro) == 0 && jnox_shm_close(decoy) == 0);
    /* Each committed byte now owns the sole reference to a different backing object. */
    for (unsigned i = 0; i < 2; ++i) {
        int temporary = jnox_shm_create(16 + i);
        CHECK(temporary >= 0);
        CHECK(send_right(data, temporary, i ? "R" : "T", 1) == 1);
        CHECK(jnox_shm_close(temporary) == 0);
    }
    CHECK(put_byte(gate, 'L') && get_byte(gate) == 'e');
    CHECK(close(data) == 0 && close(gate) == 0);
    W[CLOSED] = 1;
    W[STAGE] = 3;
    return 71;
}

static int unrelated(void)
{
    int fd = connection('A');
    CHECK(fd >= 0);
    int guess = get_byte(fd);
    int passed = get_byte(fd);
    CHECK(guess >= 0 && passed >= 0 && passed != guess);
    W[SOURCE_FD] = guess;
    W[PASSED_FD] = passed;
    CHECK(jnox_shm_map(passed, JNOX_SHM_READ) == JNOX_SHM_MAP_FAILED);
    W[STALE_ERR] = errno;
    CHECK(W[STALE_ERR] == EBADF && jnox_shm_size(guess) == -1);
    W[OTHER_ERR] = errno;
    CHECK(W[OTHER_ERR] == EBADF && send_right(fd, passed, "!", 1) == -1);
    W[DENIED_SEND] = errno;
    CHECK(W[DENIED_SEND] == EBADF);
    int own = jnox_shm_create(4);
    CHECK(own == guess);
    W[LOCAL_FD] = own;
    W[REUSED_SIZE] = jnox_shm_size(own);
    volatile uint32_t *map = jnox_shm_map(own, JNOX_SHM_READ | JNOX_SHM_WRITE);
    CHECK(map != JNOX_SHM_MAP_FAILED && *map == 0 && W[REUSED_SIZE] == 4);
    W[MAP_VA] = (uintptr_t)map;
    *map = 0x10203040;
    CHECK(jnox_shm_unmap((void *)map) == 0 && jnox_shm_close(own) == 0);
    W[STAGE] = 1;
    CHECK(put_byte(fd, 'V') && get_byte(fd) == 'x');
    int gate = connection('K');
    CHECK(gate >= 0 && get_byte(gate) == 'q');
    /* The first endpoint still owns an unread SCM_RIGHTS byte. Process exit
     * tears both sockets down; EOF on the independent gate witnesses that teardown. */
    W[STAGE] = 3;
    return 72;
}

static int pool_full(int index)
{
    int extra = jnox_shm_create(4);
    W[index] = extra < 0 ? errno : 0;
    return extra == -1 && W[index] == ENOSPC;
}

static int consumer(void)
{
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    CHECK(listener >= 0 && bind(listener, (const struct sockaddr *)&endpoint, sizeof(endpoint)) == 0);
    CHECK(listen(listener, 2) == 0 && jnox_display_init() == 0);
    int guard = jnox_shm_create(4);
    CHECK(guard >= 0);
    int guard_ro = jnox_shm_dup_readonly(guard);
    CHECK(guard_ro >= 0);
    int data = -1, producer_gate = -1, stranger = -1;
    for (unsigned i = 0; i < 3; ++i) {
        int fd = accept(listener, 0, 0);
        CHECK(fd >= 0);
        int id = get_byte(fd);
        if (id == 'D' && data < 0) data = fd;
        else if (id == 'P' && producer_gate < 0) producer_gate = fd;
        else if (id == 'A' && stranger < 0) stranger = fd;
        else CHECK(0);
    }
    /* The producer cannot send a data byte until 's'. */
    CHECK(cancel_empty_rights_receive(data));
    CHECK(jnox_shm_size(98) == -1 && errno == EBADF);
    CHECK(put_byte(producer_gate, 's'));
    CHECK(get_byte(producer_gate) == 'Q');
    int source = get_byte(producer_gate);
    int passed = get_byte(producer_gate);
    CHECK(source == guard && passed == guard_ro);
    W[SOURCE_FD] = source;
    W[PASSED_FD] = passed;
    CHECK(put_byte(stranger, source) && put_byte(stranger, passed) && get_byte(stranger) == 'V');
    W[STAGE] = 10;
    CHECK(wait_key('r'));
    char bytes[8];
    int received, flags;
    size_t used;
    unsigned cap = sizeof(union control);
    /* Ask for eight bytes; the first ancillary marker ends the plain prefix. */
    CHECK(receive(data, bytes, sizeof(bytes), cap, &received, &flags, &used) == 2);
    CHECK(bytes[0] == 'p' && bytes[1] == 'r' && received == -1 && !flags && !used);
    W[PREFIX] = 0x7072;
    CHECK(receive(data, bytes, 1, cap, &received, &flags, &used) == 1 && bytes[0] == 'F');
    CHECK(!flags && received >= 0 && received != source && received != passed && used >= CMSG_LEN(sizeof(int)));
    W[LOCAL_FD] = received;
    W[SOURCE_SIZE] = jnox_shm_size(received);
    CHECK(jnox_shm_map(received, JNOX_SHM_READ | JNOX_SHM_WRITE) == JNOX_SHM_MAP_FAILED);
    W[OTHER_ERR] = errno;
    CHECK(W[OTHER_ERR] == EACCES);
    const volatile uint32_t *map = jnox_shm_map(received, JNOX_SHM_READ);
    CHECK(W[SOURCE_SIZE] == BYTES && map != JNOX_SHM_MAP_FAILED);
    W[MAP_VA] = (uintptr_t)map;
    W[STAGE] = 11;
    CHECK(wait_key('i'));
    CHECK(jnox_shm_close(received) == 0 && jnox_shm_map(received, JNOX_SHM_READ) == JNOX_SHM_MAP_FAILED);
    W[STALE_ERR] = errno;
    CHECK(W[STALE_ERR] == EBADF && jnox_shm_close(guard_ro) == 0 && jnox_shm_close(guard) == 0);
    CHECK(receive(data, bytes, sizeof(bytes), cap, &received, &flags, &used) == 3);
    CHECK(bytes[0] == 'D' && bytes[1] == '1' && bytes[2] == 'z' && received == -1 && !flags && !used);
    W[SUFFIX] = 0x44317a;
    CHECK(put_byte(producer_gate, 'H') && get_byte(producer_gate) == 'h');
    CHECK(present(map, 0));
    W[STAGE] = 1;
    CHECK(wait_key('u') && put_byte(producer_gate, 'u') && get_byte(producer_gate) == 'U');
    CHECK(present(map, 1));
    W[STAGE] = 2;
    CHECK(wait_key('q') && jnox_shm_unmap((void *)map) == 0);
    CHECK(jnox_shm_unmap((void *)map) == -1);
    W[UNMAP_ERR] = errno;
    CHECK(W[UNMAP_ERR] == EINVAL && put_byte(producer_gate, 'q') && get_byte(producer_gate) == 'L');

    int hold[4];
    for (unsigned i = 0; i < 2; ++i) { hold[i] = jnox_shm_create(BYTES); CHECK(hold[i] >= 0); }
    CHECK(pool_full(POOL_WITH_TWO));
    CHECK(receive(data, bytes, 1, 1, &received, &flags, &used) == 1 && bytes[0] == 'T');
    W[TRUNC_FLAGS] = flags;
    W[TRUNC_LENGTH] = used;
    CHECK(received == -1 && flags == MSG_CTRUNC && used == 0);
    hold[2] = jnox_shm_create(BYTES);
    CHECK(hold[2] >= 0);
    W[PLAIN_BYTE] = get_byte(data);
    CHECK(W[PLAIN_BYTE] == 'R');
    hold[3] = jnox_shm_create(BYTES);
    CHECK(hold[3] >= 0 && pool_full(POOL_WITH_FOUR));
    /* All four backing objects can be remapped. Recycled pages must be zero. */
    for (unsigned i = 0; i < 4; ++i) {
        const volatile uint32_t *p = jnox_shm_map(hold[i], JNOX_SHM_READ);
        CHECK(p != JNOX_SHM_MAP_FAILED);
        for (unsigned j = 0; j < WIDTH * HEIGHT; ++j) CHECK(p[j] == 0);
        CHECK(jnox_shm_unmap((void *)p) == 0);
        W[RECLAIMED]++;
    }
    CHECK(close(data) == 0 && put_byte(producer_gate, 'e') && close(producer_gate) == 0);
    CHECK(put_byte(stranger, 'x'));
    int exit_gate = accept(listener, 0, 0);
    CHECK(exit_gate >= 0 && get_byte(exit_gate) == 'K');
    CHECK(send_right(stranger, hold[3], "X", 1) == 1 && jnox_shm_close(hold[3]) == 0);
    CHECK(pool_full(POOL_ON_EXIT) && put_byte(exit_gate, 'q'));
    CHECK(recv(exit_gate, bytes, 1, 0) == 0);
    W[EXIT_EOF] = 1;
    hold[3] = jnox_shm_create(BYTES);
    CHECK(hold[3] >= 0);
    for (unsigned i = 0; i < 4; ++i) CHECK(jnox_shm_close(hold[i]) == 0);
    CHECK(close(exit_gate) == 0 && close(stranger) == 0 && close(listener) == 0);
    W[CLOSED] = 1;
    W[STAGE] = 3;
    return 70;
}

int main(int argc, char **argv)
{
    CHECK(jnox_socket_broker(3) == 0);
    if (argc == 1) { W[ROLE] = 0; return consumer(); }
    CHECK(argc == 2 && argv && argv[1] && argv[1][0] && !argv[1][1]);
    if (argv[1][0] == 'p') { W[ROLE] = 1; return producer(); }
    if (argv[1][0] == 'a') { W[ROLE] = 2; return unrelated(); }
    CHECK(0);
}
