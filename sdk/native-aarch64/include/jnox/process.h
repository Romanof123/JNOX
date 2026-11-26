#ifndef JNOX_PROCESS_H
#define JNOX_PROCESS_H
#include <stddef.h>

/* Native single-core shell catalog profile. A redirect of -1 selects the
 * ordinary stream. argv must be terminated by a null pointer. */
int jnox_spawn(const char *program, char *const argv[], int stdin_fd, int stdout_fd);
/* Returns pid on success, writing the signed native exit code if status != NULL. */
int jnox_waitpid(int pid, int *status);
/* Slot zero only. A single successful call copies the complete text snapshot,
 * returns its byte length, and does not append a NUL. */
#define JNOX_DMESG_BYTES 512
int jnox_dmesg(char *buffer, size_t length);
/* Single nonblocking UART byte, or -1 with errno (EAGAIN when empty). */
int jnox_shell_getchar(void);
#endif
