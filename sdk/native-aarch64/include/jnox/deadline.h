#ifndef JNOX_DEADLINE_H
#define JNOX_DEADLINE_H

/* Return the next activation ordinal after resumption, or -1 with errno. */
int jnox_deadline_service_wait_next(void);

#endif
