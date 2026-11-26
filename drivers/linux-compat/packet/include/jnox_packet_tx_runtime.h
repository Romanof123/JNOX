/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_PACKET_TX_RUNTIME_H
#define JNOX_PACKET_TX_RUNTIME_H
#include <linux/types.h>
struct netdev_queue;
enum jnox_tx_fault {
    JNOX_TX_CLOCK = 1, JNOX_TX_DEBUG, JNOX_TX_COOKIE, JNOX_TX_BQL,
    JNOX_TX_QUEUE, JNOX_TX_NAPI, JNOX_TX_TIMESTAMP, JNOX_TX_ROUTE,
    JNOX_TX_CONNTRACK, JNOX_TX_REFCOUNT, JNOX_TX_XDP, JNOX_TX_XSK
};
extern volatile unsigned int jnox_packet_tx_fault_v1;
extern volatile unsigned long jnox_packet_tx_debug_count_v1;
int jnox_packet_tx_clock_v1(void);
bool jnox_packet_tx_take_wake_v1(struct netdev_queue *q);
#endif
