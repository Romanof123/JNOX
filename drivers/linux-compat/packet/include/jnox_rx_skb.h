/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_RX_SKB_H
#define JNOX_RX_SKB_H
int jnox_rx_skb_read_v1(void *out, unsigned int capacity);
unsigned int jnox_rx_skb_pending_v1(void);
/* Built, appended, classified, GRO_NORMAL, dequeued, released, pending,
 * explicit non-consume drop reason. Plain consume_skb is a release. */
void jnox_rx_skb_stats_v1(unsigned long long out[8]);
#endif
