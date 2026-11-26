#include <errno.h>
#include <jnox/process.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

enum { command_bytes = 192, arguments = 8 };
static char input[command_bytes];
static char *words[2][arguments + 1];
/* Parsed, pipe created, PIDs, wait results, raw child statuses. */
volatile int32_t shell_result[8];

static int parse(const char *command)
{
    unsigned side = 0;
    unsigned used[2] = {0, 0};
    unsigned n = 0;
    while (command[n]) {
        if (n + 1 == sizeof(input))
            return -1;
        input[n] = command[n];
        ++n;
    }
    input[n] = 0;
    int in_word = 0;
    for (unsigned i = 0; i < n; ++i) {
        char c = input[i];
        if (c == ' ' || c == '\t' || c == '|') {
            in_word = 0;
            input[i] = 0;
            if (c == '|') {
                if (side || !used[0])
                    return -1;
                side = 1;
            }
        } else if (!in_word) {
            if (used[side] == arguments)
                return -1;
            words[side][used[side]++] = &input[i];
            in_word = 1;
        }
    }
    words[0][used[0]] = 0;
    words[1][used[1]] = 0;
    if (!used[0] || (side && !used[1]))
        return -1;
    return (int)side;
}

static int equal(const char *left, const char *right)
{
    while (*left && *left == *right) {
        ++left;
        ++right;
    }
    return *left == *right;
}

static int pipeline(void)
{
    shell_result[0] = 1;
    int ends[2];
    if (pipe(ends) != 0)
        return 3;
    shell_result[1] = 1;
    int first = jnox_spawn(words[0][0], words[0], -1, ends[1]);
    shell_result[2] = first;
    int second = first < 0 ? -1 : jnox_spawn(words[1][0], words[1], ends[0], -1);
    shell_result[3] = second;
    /* Release both parent copies before waiting, including after a failed spawn. */
    int closed_reader = close(ends[0]);
    int closed_writer = close(ends[1]);
    int left_status = -1;
    int right_status = -1;
    if (first >= 0)
        shell_result[4] = jnox_waitpid(first, &left_status);
    if (second >= 0)
        shell_result[5] = jnox_waitpid(second, &right_status);
    shell_result[6] = left_status;
    shell_result[7] = right_status;
    if (first < 0 || second < 0)
        return 127;
    if (closed_reader || closed_writer ||
        shell_result[4] != first || shell_result[5] != second)
        return 4;
    return right_status;
}

static int output(int fd, const char *bytes, size_t count)
{
    while (count) {
        ssize_t n = write(fd, bytes, count);
        if (n <= 0)
            return -1;
        bytes += n;
        count -= (size_t)n;
    }
    return 0;
}

static int run(const char *command)
{
    int sides = parse(command);
    if (sides == 1)
        return pipeline();
    if (sides < 0 || words[0][1] || !equal(words[0][0], "dmesg"))
        return 2;
    char snapshot[JNOX_DMESG_BYTES];
    int n = jnox_dmesg(snapshot, sizeof(snapshot));
    if (n < 0) {
        if (errno == EACCES) {
            const char error[] = "dmesg: permission denied\n";
            output(STDERR_FILENO, error, sizeof(error) - 1);
        }
        return 1;
    }
    return output(STDOUT_FILENO, snapshot, (size_t)n) == 0 ? 0 : 1;
}

static int next_byte(void)
{
    for (;;) {
        int byte = jnox_shell_getchar();
        if (byte >= 0 || errno != EAGAIN)
            return byte;
        struct timespec wake;
        if (clock_gettime(CLOCK_MONOTONIC, &wake) < 0)
            return -1;
        wake.tv_nsec += 10000000;
        if (wake.tv_nsec >= 1000000000) {
            ++wake.tv_sec;
            wake.tv_nsec -= 1000000000;
        }
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &wake, 0))
            return -1;
    }
}

static int interactive(void)
{
    char line[command_bytes];
    int after_cr = 0;
    for (;;) {
        unsigned used = 0;
        int overlong = 0;
        if (output(STDOUT_FILENO, "jnox$ ", 6) < 0)
            return 1;
        for (;;) {
            int byte = next_byte();
            if (byte < 0)
                return 1;
            if (after_cr && byte == '\n') {
                after_cr = 0;
                continue;
            }
            after_cr = byte == '\r';
            if (after_cr || byte == '\n')
                break;
            if (overlong)
                continue;
            if (byte == 8 || byte == 127) {
                if (used) {
                    --used;
                    output(STDOUT_FILENO, "\b \b", 3);
                }
            } else if ((byte >= 32 && byte < 127) || byte == '\t') {
                if (used + 1 >= sizeof(line)) {
                    overlong = 1;
                    continue;
                }
                line[used++] = (char)byte;
                output(STDOUT_FILENO, &line[used - 1], 1);
            }
        }
        output(STDOUT_FILENO, "\n", 1);
        if (overlong) {
            output(STDERR_FILENO, "line too long\n", 14);
            continue;
        }
        line[used] = 0;
        if (equal(line, "exit"))
            return 0;
        if (used && run(line) != 0)
            output(STDERR_FILENO, "command failed\n", 15);
    }
}

int main(int argc, char **argv)
{
    if (argc == 2 && equal(argv[1], "-i"))
        return interactive();
    if (argc < 2)
        return 2;
    int status = 0;
    for (int i = 1; i < argc; ++i)
        status = run(argv[i]);
    return status;
}
