/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_ORIGINAL_TX_H
#define JNOX_ORIGINAL_TX_H
int jnox_original_tx_init(void);
int jnox_original_tx_frame(const unsigned char *frame, unsigned int len, unsigned int sequence);
int jnox_original_tx_complete(unsigned int sequence);
#ifdef JNOX_PACKET_DUPLEX_ABI
int jnox_original_tx_defer(const unsigned char *frame, unsigned int len, unsigned int sequence);
int jnox_original_tx_flush(unsigned int sequence);
extern volatile unsigned long long jnox_duplex_tx[16];
#endif
struct sk_buff;
int jnox_packet_retire_skb_v1(struct sk_buff *skb, unsigned int reason);
#endif
