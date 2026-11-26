/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_PACKET_LINUX_SLAB_H
#define JNOX_PACKET_LINUX_SLAB_H
#include_next <linux/slab.h>
#ifdef JNOX_PACKET_RX_POLL_ABI
void *jnox_rx_queue_calloc_v1(size_t count, size_t size, gfp_t flags);
#undef kcalloc
#define kcalloc(count,size,flags) jnox_rx_queue_calloc_v1((count),(size),(flags))
#undef kzalloc
#define kzalloc(size,flags) jnox_rx_queue_calloc_v1(1,(size),(flags))
#endif
#endif
