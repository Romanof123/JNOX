/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_RX_VIRTQUEUE_H
#define JNOX_RX_VIRTQUEUE_H
long jnox_rx_start_v1(void);
void jnox_rx_reset_complete_v1(unsigned long retired_token);
unsigned long jnox_rx_posted_address_v1(unsigned int id);
void jnox_rx_vq_stats(unsigned long long out[8]);
#endif
