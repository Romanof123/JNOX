#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <linux/capability.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

enum { PAGE_BYTES = 4096, RECORD = 24, HEADER = 16, DATA = 8, MAX_MS = 10000 };
#define GATEWAY_IPA 0x0b000000
#define DT "/sys/firmware/devicetree/base/jnox-gateway@b000000/"

static void fail(const char *operation)
{
    perror(operation);
    exit(1);
}

static void property(const char *path, const void *expected, size_t size)
{
    unsigned char buf[64];
    int fd = open(path, O_RDONLY | O_CLOEXEC);
    if (fd < 0) fail(path);
    ssize_t count = read(fd, buf, sizeof buf);
    if (count < 0) fail(path);
    if ((size_t)count != size || memcmp(buf, expected, size)) {
        fprintf(stderr, "BG unexpected DT property %s\n", path);
        exit(1);
    }
    close(fd);
}

static volatile uint32_t *open_page(void)
{
    const uint32_t reg[] = {0, htonl(GATEWAY_IPA), 0, htonl(PAGE_BYTES)};
    property(DT "compatible", "jnox,gateway-v1", sizeof("jnox,gateway-v1"));
    property(DT "reg", reg, sizeof reg);
    if (sysconf(_SC_PAGESIZE) != PAGE_BYTES) exit(1);
    int fd = open("/dev/mem", O_RDWR | O_SYNC | O_CLOEXEC);
    if (fd < 0) fail("open /dev/mem");
    void *page = mmap(NULL, PAGE_BYTES, PROT_READ | PROT_WRITE, MAP_SHARED, fd, GATEWAY_IPA);
    if (page == MAP_FAILED) fail("mmap gateway page");
    if (close(fd)) fail("close /dev/mem");
    return page;
}

static void drop_privileges(void)
{
    if (setgroups(0, NULL) || setresgid(65534, 65534, 65534) ||
        setresuid(65534, 65534, 65534) || prctl(PR_SET_NO_NEW_PRIVS, 1L, 0L, 0L, 0L))
        fail("drop privileges");
    struct __user_cap_header_struct hdr = { .version = _LINUX_CAPABILITY_VERSION_3 };
    struct __user_cap_data_struct caps[2] = {{0}};
    if (syscall(SYS_capset, &hdr, caps) || syscall(SYS_capget, &hdr, caps)) fail("capabilities");
    if (caps[0].permitted || caps[1].permitted || caps[0].effective || caps[1].effective ||
        caps[0].inheritable || caps[1].inheritable || geteuid() != 65534 || getegid() != 65534 ||
        getgroups(0, NULL) != 0 || prctl(PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L) != 1) exit(1);
    if (open("/dev/mem", O_RDWR | O_CLOEXEC) >= 0 || (errno != EACCES && errno != EPERM)) exit(1);
    puts("BG PRIV uid=65534 gid=65534 caps=0 nnp=1 devmem=denied map=0x0b000000+4096");
}

static void barrier(void)
{
    __asm__ volatile("dmb sy" ::: "memory");
}

/* The shared page permits only aligned word accesses. Byte operations and
 * wire byte order conversion operate on our private snapshot. */
static uint32_t snapshot(volatile uint32_t *lane, uint32_t previous, unsigned char packet[RECORD])
{
    uint32_t seq = lane[0];
    if (seq == 0 || (seq & 1) || seq == previous) return 0;
    barrier();
    for (size_t i = 0; i < RECORD / 4; i++) {
        uint32_t word = lane[1 + i];
        memcpy(packet + i * 4, &word, 4);
    }
    barrier();
    return lane[0] == seq ? seq : 0;
}

static void publish(volatile uint32_t *lane, uint32_t seq, const unsigned char packet[RECORD])
{
    lane[0] = seq - 1;
    barrier();
    for (size_t i = 0; i < RECORD / 4; i++) {
        uint32_t word;
        memcpy(&word, packet + i * 4, 4);
        lane[1 + i] = word;
    }
    barrier();
    lane[0] = seq;
    barrier();
}

static uint32_t field(const unsigned char *p)
{
    uint32_t word;
    memcpy(&word, p, 4);
    return ntohl(word);
}

static void record(const char *label, const unsigned char *packet)
{
    char hex[2 * RECORD + 1];
    for (size_t i = 0; i < RECORD; i++) snprintf(hex + 2 * i, 3, "%02x", packet[i]);
    printf("BG %s %s\n", label, hex);
}

static int64_t milliseconds(void)
{
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now)) fail("clock");
    return (int64_t)now.tv_sec * 1000 + now.tv_nsec / 1000000;
}

static int wait_fd(int fd, short events, int64_t end)
{
    struct pollfd pfd = { .fd = fd, .events = events };
    for (;;) {
        int64_t left = end - milliseconds();
        if (left <= 0) return 0;
        int rc = poll(&pfd, 1, (int)left);
        if (rc < 0 && errno == EINTR) continue;
        return rc > 0 && (pfd.revents & (events | POLLHUP)) && milliseconds() < end;
    }
}

static int transfer(int fd, unsigned char *bytes, size_t size, short event, int64_t end)
{
    while (size && wait_fd(fd, event, end)) {
        ssize_t n = event == POLLIN ? read(fd, bytes, size) : write(fd, bytes, size);
        if (n < 0 && (errno == EINTR || errno == EAGAIN)) continue;
        if (n <= 0) return 0;
        size -= n;
        bytes += n;
    }
    return size == 0;
}

static int exchange(int listener, unsigned char packet[RECORD], int64_t end)
{
    int client = -1;
    while (wait_fd(listener, POLLIN, end)) {
        client = accept4(listener, NULL, NULL, SOCK_CLOEXEC | SOCK_NONBLOCK);
        if (client >= 0) break;
        if (errno != EINTR && errno != EAGAIN) return 0;
    }
    if (client < 0) return 0;
    int valid = transfer(client, packet, RECORD, POLLOUT, end) && shutdown(client, SHUT_WR) == 0 &&
                transfer(client, packet + HEADER, DATA, POLLIN, end);
    int exact = 0;
    while (valid && wait_fd(client, POLLIN, end)) {
        unsigned char extra;
        ssize_t n = read(client, &extra, 1);
        if (n < 0 && (errno == EINTR || errno == EAGAIN)) continue;
        exact = n == 0;
        break;
    }
    close(client);
    return exact;
}

int main(void)
{
    setvbuf(stdout, NULL, _IOLBF, 0);
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR) fail("signal");
    volatile uint32_t *page = open_page();
    int listener = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (listener < 0) fail("socket");
    int yes = 1;
    if (setsockopt(listener, SOL_SOCKET, SO_BINDTODEVICE, "eth0", sizeof("eth0")) ||
        setsockopt(listener, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof yes)) fail("interface");
    drop_privileges();
    struct sockaddr_in addr = { .sin_family = AF_INET, .sin_port = htons(8080) };
    if (inet_pton(AF_INET, "10.0.2.15", &addr.sin_addr) != 1 ||
        bind(listener, (struct sockaddr *)&addr, sizeof addr) || listen(listener, 1)) fail("listen");
    puts("BG READY");
    uint32_t seen = 0, generation = 0, last_id = 0;
    for (;;) {
        unsigned char packet[RECORD];
        uint32_t seq = snapshot(page, seen, packet);
        if (!seq) {
            const struct timespec delay = { .tv_nsec = 1000000 };
            nanosleep(&delay, NULL);
            continue;
        }
        seen = seq;
        uint32_t gen = field(packet + 4), id = field(packet + 8), budget = field(packet + 12);
        if (memcmp(packet, "JG\x01\x01", 4) || !gen || gen < generation || !id ||
            (gen == generation && id <= last_id) || !budget || budget > MAX_MS) continue;
        int64_t end = milliseconds() + budget;
        generation = gen;
        last_id = id;
        record("REQ", packet);
        if (!exchange(listener, packet, end)) {
            printf("BG TIMEOUT %u %u\n", gen, id);
            continue;
        }
        if (page[0] != seq) continue;
        packet[3] = 2;
        publish(page + 64 / sizeof(uint32_t), seq, packet);
        record("REP", packet);
    }
}
