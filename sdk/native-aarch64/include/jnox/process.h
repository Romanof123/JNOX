#ifndef JNOX_PROCESS_H
#define JNOX_PROCESS_H

/* Native single-core shell catalog profile. A redirect of -1 selects the
 * ordinary stream. argv must be terminated by a null pointer. */
int jnox_spawn(const char *program, char *const argv[], int stdin_fd, int stdout_fd);
/* Returns pid on success, writing the signed native exit code if status != NULL. */
int jnox_waitpid(int pid, int *status);
#endif
