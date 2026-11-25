#include <unistd.h>
#include <errno.h>
volatile unsigned char anon_data[8];
volatile int anon_eof, anon_epipe_rc, anon_epipe_errno, anon_done;
static int collect(int fd, unsigned offset, const char *expected)
{
    unsigned got = 0;
    while (got < 4) {
        unsigned char bytes[4];
        ssize_t n = read(fd, bytes, 4 - got);
        if (n <= 0) return 0;
        for (ssize_t i = 0; i < n; ++i) {
            if (bytes[i] != (unsigned char)expected[got]) return 0;
            anon_data[offset + got++] = bytes[i];
        }
    }
    return 1;
}
int main(void)
{
    char last;
    int held = dup(0);
    if (held < 0 || close(0) || !collect(held, 0, "rain")) return 90;
    if (write(4, "a", 1) != 1 || !collect(held, 4, "fall")) return 91;
    anon_eof = (int)read(held, &last, 1);
    if (anon_eof != 0 || close(held)) return 92;
    // The writer closed the reverse read end before publishing the final bytes.
    anon_epipe_rc = (int)write(4, "x", 1);
    anon_epipe_errno = errno;
    if (anon_epipe_rc != -1 || anon_epipe_errno != EPIPE || close(4)) return 93;
    anon_done = 1;
    return 65;
}
