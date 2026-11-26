/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_TX_VIRTQUEUE_H
#define JNOX_TX_VIRTQUEUE_H
struct sk_buff;
struct virtqueue;
struct virtqueue *jnox_packet_active_vq(unsigned int index);
unsigned long jnox_packet_token(void);
int jnox_tx_register_skb(struct sk_buff *skb, const void *linear_base, unsigned int linear_capacity);
int jnox_tx_unregister_skb(struct sk_buff *skb);
void jnox_tx_vq_stats(unsigned long long out[8]);
#endif
