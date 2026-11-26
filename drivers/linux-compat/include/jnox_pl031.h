#ifndef JNOX_PL031_H
#define JNOX_PL031_H
#include <stdint.h>

struct jnox_pl031_stats {
    uint32_t periphid, cellid, probes, removes, irq_calls, irq_events, irq_flags, resources;
};
int jnox_pl031_bind(void);
int jnox_pl031_read_time(uint32_t *seconds);
int jnox_pl031_set_time(uint32_t seconds);
int jnox_pl031_read_alarm(uint32_t *seconds, unsigned *enabled, unsigned *pending);
int jnox_pl031_set_alarm(uint32_t seconds, unsigned enabled);
int jnox_pl031_poll_irq(void);
int jnox_pl031_unbind(void);
void jnox_pl031_stats(struct jnox_pl031_stats *out);
#endif
