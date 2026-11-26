/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_ORIGINAL_TX_H
#define JNOX_ORIGINAL_TX_H
int jnox_original_tx_init(void);
int jnox_original_tx_frame(const unsigned char *frame, unsigned int len, unsigned int sequence);
int jnox_original_tx_complete(unsigned int sequence);
struct sk_buff;
int jnox_packet_retire_skb_v1(struct sk_buff *skb, unsigned int reason);
#endif
