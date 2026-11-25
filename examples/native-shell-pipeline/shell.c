#include <jnox/process.h>
#include <stdint.h>
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
    return side && used[1] ? 0 : -1;
}

int main(int argc, char **argv)
{
    if (argc != 2 || parse(argv[1]) != 0)
        return 2;
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
