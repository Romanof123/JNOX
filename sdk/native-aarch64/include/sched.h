#ifndef JNOX_SCHED_H
#define JNOX_SCHED_H

#define SCHED_FIFO 1
#define SCHED_RR 2
struct sched_param { int sched_priority; };

int sched_yield(void);

#endif
