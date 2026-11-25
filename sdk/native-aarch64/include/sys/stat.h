#ifndef JNOX_SYS_STAT_H
#define JNOX_SYS_STAT_H
typedef unsigned int mode_t;
int mkfifo(const char *path, mode_t mode);
#endif
