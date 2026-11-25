#include <fcntl.h>
#include <stdint.h>
#include <unistd.h>

/* File bytes read, bytes written to stdout, reads returning EOF. */
volatile uint64_t cat_result[3];

int main(int argc, char **argv)
{
    if (argc != 2)
        return 2;
    int fd = open(argv[1], O_RDONLY);
    if (fd < 0)
        return 1;
    char data[53];
    for (;;) {
        ssize_t n = read(fd, data, sizeof(data));
        if (n < 0) {
            close(fd);
            return 1;
        }
        if (!n) {
            ++cat_result[2];
            break;
        }
        cat_result[0] += (unsigned)n;
        for (ssize_t sent = 0; sent < n;) {
            ssize_t k = write(STDOUT_FILENO, data + sent, (size_t)(n - sent));
            if (k <= 0) {
                close(fd);
                return 1;
            }
            sent += k;
            cat_result[1] += (unsigned)k;
        }
    }
    return close(fd) == 0 ? 0 : 1;
}
