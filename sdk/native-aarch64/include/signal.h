#ifndef JNOX_SIGNAL_H
#define JNOX_SIGNAL_H
#include <unistd.h>

/* Positive PID and signal zero query process presence and permission. */
int kill(pid_t pid, int sig);
#endif
