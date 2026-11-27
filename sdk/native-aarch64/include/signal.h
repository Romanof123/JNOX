#ifndef JNOX_SIGNAL_H
#define JNOX_SIGNAL_H
#include <unistd.h>

/* Numbers shared with the AArch64 Picolibc graphics profile. */
#define SIGHUP   1
#define SIGINT   2
#define SIGPIPE 13
#define SIGTERM 15
#define SIGUSR1 30
/* Positive PID and signal zero query process presence and permission. */
int kill(pid_t pid, int sig);
#endif
