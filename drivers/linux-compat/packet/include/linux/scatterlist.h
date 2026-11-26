/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_PACKET_LINUX_SCATTERLIST_H
#define JNOX_PACKET_LINUX_SCATTERLIST_H
#include_next <linux/scatterlist.h>
#ifdef JNOX_PACKET_TX_ABI
/* CPU-local scatterlist pages describe registered skb storage. EL1 owns the
 * unrelated DMA pages, so Linux linear-map virt_to_page cannot translate it. */
void jnox_sg_set_buf_v1(struct scatterlist *sg, const void *buf, unsigned int len);
#define sg_set_buf(sg, buf, len) jnox_sg_set_buf_v1((sg), (buf), (len))
#endif
#endif
