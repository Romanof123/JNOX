#include <stdint.h>
#include <unistd.h>

/* Stdin bytes read, newline count, reads returning EOF, printed bytes. */
volatile uint64_t wc_result[4];

int main(int argc, char **argv)
{
    if (argc != 2 || argv[1][0] != '-' || argv[1][1] != 'l' || argv[1][2])
        return 2;
    char data[37];
    for (;;) {
        ssize_t n = read(STDIN_FILENO, data, sizeof(data));
        if (n < 0)
            return 1;
        if (!n) {
            ++wc_result[2];
            break;
        }
        wc_result[0] += (unsigned)n;
        for (ssize_t i = 0; i < n; ++i)
            wc_result[1] += data[i] == '\n';
    }
    char output[21];
    size_t start = sizeof(output);
    output[--start] = '\n';
    uint64_t value = wc_result[1];
    do {
        output[--start] = (char)('0' + value % 10);
        value /= 10;
    } while (value);
    while (start < sizeof(output)) {
        ssize_t n = write(STDOUT_FILENO, output + start, sizeof(output) - start);
        if (n <= 0)
            return 1;
        start += (size_t)n;
        wc_result[3] += (size_t)n;
    }
    return 0;
}
