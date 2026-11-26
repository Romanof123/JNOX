/* The PL011 connection carries the copied RPC. eth0 performs the device operation. */
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

enum { RECORD = 24, HEADER = 16, DATA = 8, MAX_MS = 10000, SERIAL_MS = 1000 };

static int64_t now_ms(void)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t) < 0)
        exit(2);
    return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000;
}

/* A zero deadline is used only while waiting for the start of a serial line. */
static int ready(int fd, short events, int64_t deadline)
{
    struct pollfd p = { .fd = fd, .events = events };
    for (;;) {
        int64_t remaining = deadline ? deadline - now_ms() : -1;
        if (deadline && remaining <= 0)
            return 0;
        int rc = poll(&p, 1, (int)remaining);
        if (rc < 0 && errno == EINTR)
            continue;
        if (rc <= 0 || (deadline && now_ms() >= deadline))
            return 0;
        return (p.revents & (events | POLLHUP)) != 0;
    }
}

static int transfer(int fd, unsigned char *bytes, size_t n, int writing, int64_t deadline)
{
    while (n > 0 && ready(fd, writing ? POLLOUT : POLLIN, deadline)) {
        ssize_t nbytes = writing ? write(fd, bytes, n) : read(fd, bytes, n);
        if (nbytes < 0 && (errno == EINTR || errno == EAGAIN))
            continue;
        if (nbytes <= 0)
            return 0;
        bytes += nbytes;
        n -= (size_t)nbytes;
    }
    return n == 0;
}

static int hex(char c)
{
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    return -1;
}

/* An oversized or slow frame is discarded through its newline. A fresh frame
 * cannot borrow bytes from a discarded prefix or suffix. */
static int receive_request(int fd, unsigned char packet[RECORD], int64_t *started)
{
    char line[6 + 2 * RECORD];
    size_t used = 0;
    int invalid = 0;
    *started = 0;
    for (;;) {
        int64_t until = (*started && !invalid) ? *started + SERIAL_MS : 0;
        if (!ready(fd, POLLIN, until)) {
            if (!until || now_ms() < until)
                return -1;
            invalid = 1;
            continue;
        }
        char c;
        ssize_t n = read(fd, &c, 1);
        if (n < 0 && (errno == EINTR || errno == EAGAIN))
            continue;
        if (n != 1)
            return -1;
        if (!*started)
            *started = now_ms();
        if (c == '\n')
            break;
        if (used == sizeof line)
            invalid = 1;
        if (!invalid)
            line[used++] = c;
    }
    if (invalid || used != sizeof line || memcmp(line, "GWREQ ", 6) != 0)
        return 0;
    for (size_t i = 0; i < RECORD; ++i) {
        int hi = hex(line[6 + 2*i]), lo = hex(line[7 + 2*i]);
        if (hi < 0 || lo < 0)
            return 0;
        packet[i] = (unsigned char)(16*hi + lo);
    }
    return 1;
}

static int send_record(int fd, const char *prefix, const unsigned char packet[RECORD], int64_t until)
{
    static const char digits[] = "0123456789abcdef";
    unsigned char line[sizeof("GWACCEPT ") - 1 + 2*RECORD + 1];
    size_t len = strlen(prefix);
    memcpy(line, prefix, len);
    for (size_t i = 0; i < RECORD; ++i) {
        line[len++] = digits[packet[i] >> 4];
        line[len++] = digits[packet[i] & 15];
    }
    line[len++] = '\n';
    return transfer(fd, line, len, 1, until);
}

static uint32_t word(const unsigned char *bytes)
{
    uint32_t value;
    memcpy(&value, bytes, sizeof value);
    return ntohl(value);
}

static int open_listener(void)
{
    /* A fixed interface and local address bind this operation to the original
     * Linux NIC. QEMU publishes a single host forward on loopback. */
    int fd = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    struct sockaddr_in addr = { .sin_family = AF_INET, .sin_port = htons(8080) };
    int yes = 1;
    if (fd < 0)
        return -1;
    if (setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof yes) < 0 ||
        setsockopt(fd, SOL_SOCKET, SO_BINDTODEVICE, "eth0", sizeof("eth0")) < 0 ||
        inet_pton(AF_INET, "10.0.2.15", &addr.sin_addr) != 1 ||
        bind(fd, (struct sockaddr *)&addr, sizeof addr) < 0 || listen(fd, 1) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}

static int exchange(int listener, unsigned char packet[RECORD], int64_t until)
{
    int stream = -1;
    while (ready(listener, POLLIN, until)) {
        stream = accept4(listener, NULL, NULL, SOCK_NONBLOCK | SOCK_CLOEXEC);
        if (stream >= 0)
            break;
        if (errno != EINTR && errno != EAGAIN)
            return 0;
    }
    if (stream < 0)
        return 0;
    /* Each operation owns one stream. The host returns exactly eight bytes.
     * Require an orderly end so an oversized or truncated response cannot
     * produce a successful serial reply. */
    unsigned char extra;
    int ok = transfer(stream, packet, RECORD, 1, until) &&
             shutdown(stream, SHUT_WR) == 0 &&
             transfer(stream, packet + HEADER, DATA, 0, until);
    if (ok) {
        ok = 0;
        while (ready(stream, POLLIN, until)) {
            ssize_t n = read(stream, &extra, 1);
            if (n < 0 && (errno == EINTR || errno == EAGAIN))
                continue;
            ok = n == 0;
            break;
        }
    }
    close(stream);
    return ok;
}

int main(int argc, char **argv)
{
    if (argc != 2 || !*argv[1] || strspn(argv[1], "0123456789") != strlen(argv[1]))
        return 2;
    errno = 0;
    unsigned long parsed = strtoul(argv[1], NULL, 10);
    if (errno || parsed == 0 || parsed > UINT32_MAX)
        return 2;
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR)
        return 2;
    uint32_t generation = (uint32_t)parsed, previous_id = 0;
    int serial = open("/dev/ttyAMA0", O_RDWR | O_NOCTTY | O_NONBLOCK | O_CLOEXEC);
    struct termios tty;
    if (serial < 0 || tcgetattr(serial, &tty) < 0)
        return 3;
    cfmakeraw(&tty);
    if (tcsetattr(serial, TCSAFLUSH, &tty) < 0)
        return 3;
    int listener = open_listener();
    if (listener < 0)
        return 4;
    unsigned char announced[32];
    int size = snprintf((char *)announced, sizeof announced, "GWREADY %u\n", generation);
    if (!transfer(serial, announced, (size_t)size, 1, now_ms() + SERIAL_MS))
        return 3;
    for (;;) {
        unsigned char packet[RECORD];
        int64_t started;
        int status = receive_request(serial, packet, &started);
        if (status < 0)
            return 5;
        if (status == 0 || memcmp(packet, "JG\x01\x01", 4) != 0 || word(packet + 4) != generation)
            continue;
        uint32_t id = word(packet + 8), budget = word(packet + 12);
        if (id <= previous_id || !budget || budget > MAX_MS)
            continue;
        int64_t until = started + budget;
        if (now_ms() >= until)
            continue;
        /* Never execute the same request twice in one generation. The single
         * PL011 relay is bound to the authorized JNOX gateway client. */
        previous_id = id;
        if (!send_record(serial, "GWACCEPT ", packet, until) || !exchange(listener, packet, until))
            continue;
        packet[3] = 2;
        if (!send_record(serial, "GWREP ", packet, until))
            return 5;
    }
}
