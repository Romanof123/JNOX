#include <errno.h>
#include <signal.h>
#include <unistd.h>
#include <jnox/process.h>

int main(void)
{
    char *live[] = {"live", NULL}, *orphan[] = {"orphan", NULL};
    int lifetime[2], messages[2], status;
    unsigned char message;
    if (pipe(lifetime) || jnox_spawn(orphan[0], orphan, lifetime[0], -1) != 2) return 101;
    if (close(lifetime[0])) return 102;
    if (pipe(messages) || jnox_spawn(live[0], live, -1, messages[1]) != 1) return 103;
    if (close(messages[1]) || read(messages[0], &message, 1) != 1 || message != 'R') return 104;
    // The registered receiver outranks the parent. It must block in sleep
    // before the parent can return to EL0 and send this signal.
    errno = EBUSY;
    if (kill(1, SIGUSR1) || errno != EBUSY) return 105;
    if (read(messages[0], &message, 1) != 1 || message < 1 || message > 15) return 106;
    if (jnox_waitpid(1, &status) != 1 || status != 84) return 107;
    unsigned char receipt[] = {'S', 'G', message};
    if (write(STDOUT_FILENO, receipt, sizeof(receipt)) != sizeof(receipt)) return 108;
    if (kill(1, SIGUSR1) != -1 || errno != ESRCH) return 109;
    // Exit closes the last writer and resumes the still-live denied sibling.
    return 83;
}
