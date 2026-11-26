/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_RX_PAGE_API_H
#define JNOX_RX_PAGE_API_H
#ifndef JNOX_PACKET_RX_ABI
#error "The EL0 page aperture belongs only to the bounded RX service"
#endif
#include <linux/mm.h>
#include <linux/skbuff.h>

bool jnox_rx_page_refill_v1(unsigned int size, struct page_frag *frag, gfp_t gfp);
struct page *jnox_rx_head_page_v1(const void *address);
void *jnox_rx_page_address_v1(const struct page *page);
void jnox_rx_get_page_v1(struct page *page);
void jnox_rx_put_page_v1(struct page *page);
unsigned long jnox_rx_span_v1(const void *address, unsigned int len);
void *jnox_rx_resolve_v1(unsigned long handle, unsigned int len);

#undef page_address
#undef virt_to_head_page
#undef virt_to_page
#undef get_page
#undef put_page
#undef skb_page_frag_refill
#define page_address(p) jnox_rx_page_address_v1(p)
#define virt_to_head_page(p) jnox_rx_head_page_v1(p)
#define virt_to_page(p) jnox_rx_head_page_v1(p)
#define get_page(p) jnox_rx_get_page_v1(p)
#define put_page(p) jnox_rx_put_page_v1(p)
#define skb_page_frag_refill(s, f, g) jnox_rx_page_refill_v1((s), (f), (g))
#endif
