/* Compile with the ordinary native C SDK. Run the same binary in slots 0
 * and 1 with only slot 0 named by --console-write. */
#include <errno.h>
#include <unistd.h>

int main(int argc, char **argv)
{
    if (argc != 2 || !argv[1] || argv[1][1]) return 115;
    if (argv[1][0] == 'g') {
        static const char line[] = "JNOX CONSOLE GRANT OK\n";
        if (write(2, line, sizeof(line)-1) != sizeof(line)-1) return 116;
        errno = 0;
        if (write(0, line, 1) != -1 || errno != EBADF) return 117;
        errno = 0;
        if (write(1, (const void *)1, 7) != -1 || errno != EFAULT) return 118;
        return 73;
    }
    if (argv[1][0] != 'd') return 119;
    static const char forbidden[] = "JNOX CONSOLE LEAK\n";
    for (int fd = 1; fd <= 2; ++fd) {
        errno = 0;
        if (write(fd, forbidden, sizeof(forbidden)-1) != -1 || errno != EACCES) return 120;
    }
    return 74;
}
