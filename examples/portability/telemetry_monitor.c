/* Summarize newline-delimited sensor readings without retaining the input. */
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <unistd.h>

struct summary {
    uint64_t count;
    uint64_t sum;
    uint32_t min;
    uint32_t max;
};

static int write_all(int fd, const char *data, size_t length)
{
    while (length != 0) {
        ssize_t n = write(fd, data, length);
        if (n < 0 && errno == EINTR)
            continue;
        if (n <= 0)
            return -1;
        data += n;
        length -= (size_t)n;
    }
    return 0;
}

static size_t length_of(const char *s)
{
    size_t n = 0;
    while (s[n] != '\0')
        ++n;
    return n;
}

static int diagnostic(int status, const char *message)
{
    (void)write_all(STDERR_FILENO, message, length_of(message));
    return status;
}

static int read_samples(int fd, struct summary *s)
{
    unsigned char buf[256];
    uint32_t value = 0;
    int has_digit = 0;
    for (;;) {
        ssize_t n = read(fd, buf, sizeof buf);
        if (n < 0 && errno == EINTR)
            continue;
        if (n < 0)
            return 5;
        if (n == 0)
            break;
        for (ssize_t i = 0; i < n; ++i) {
            unsigned int c = buf[i];
            if (c >= '0' && c <= '9') {
                value = value * 10 + (c - '0');
                if (value > UINT16_MAX)
                    return 4;
                has_digit = 1;
                continue;
            }
            if (c != '\n' || !has_digit)
                return 4;
            if (s->count == UINT64_MAX || UINT64_MAX - s->sum < value)
                return 6;
            if (s->count == 0 || value < s->min)
                s->min = value;
            if (s->count == 0 || value > s->max)
                s->max = value;
            ++s->count;
            s->sum += value;
            has_digit = 0;
            value = 0;
        }
    }
    if (has_digit)
        return 4;
    return s->count == 0 ? 3 : 0;
}

static char *append_text(char *p, const char *s)
{
    while (*s != '\0')
        *p++ = *s++;
    return p;
}

static char *append_decimal(char *p, uint64_t value)
{
    char reversed[20];
    size_t n = 0;
    do {
        reversed[n++] = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    while (n != 0)
        *p++ = reversed[--n];
    return p;
}

int main(int argc, char **argv)
{
    if (argc != 2)
        return diagnostic(2, "usage: telemetry_monitor PATH\n");
    int fd;
    do {
        fd = open(argv[1], O_RDONLY);
    } while (fd < 0 && errno == EINTR);
    if (fd < 0)
        return diagnostic(5, "input error\n");

    struct summary s = {0};
    int status = read_samples(fd, &s);
    if (close(fd) < 0 && status == 0)
        status = 5;
    if (status == 3)
        return diagnostic(status, "empty input\n");
    if (status == 4)
        return diagnostic(status, "invalid sample\n");
    if (status == 5)
        return diagnostic(status, "input error\n");
    if (status == 6)
        return diagnostic(status, "summary overflow\n");

    /* Up to two 20-digit totals, two 5-digit readings, labels and LF. */
    char line[96];
    char *p = append_decimal(append_text(line, "samples="), s.count);
    p = append_decimal(append_text(p, " min="), s.min);
    p = append_decimal(append_text(p, " max="), s.max);
    p = append_decimal(append_text(p, " sum="), s.sum);
    *p++ = '\n';
    if (write_all(STDOUT_FILENO, line, (size_t)(p - line)) < 0)
        return diagnostic(7, "output error\n");
    return 0;
}
