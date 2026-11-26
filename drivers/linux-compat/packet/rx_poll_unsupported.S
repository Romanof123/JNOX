// SPDX-License-Identifier: GPL-2.0-or-later
// Version 1: stable per-import IDs; never reuse an ID for another symbol.
// Generated .set directives select only verified branch relocations.
#include "jnox_rx_fault_select.inc"

.section .bss.jnox_rx_poll_fault_id,"aw",%nobits
.balign 8
.global jnox_rx_poll_fault_id
.type jnox_rx_poll_fault_id,%object
jnox_rx_poll_fault_id:
    .zero 8
.size jnox_rx_poll_fault_id,8

.macro rx_unsupported id, symbol, reason
.ifdef jnox_enable_\id
.section .text.\symbol,"ax",%progbits
.balign 4
.global \symbol
.type \symbol,%function
\symbol:
    mov x0, #\id
    b .Lrx_stop
.size \symbol,.-\symbol
.endif
.endm

// IDs 1 and 7 are reserved; tx_runtime provides their original imports.
rx_unsupported 2, xdp_return_frame_rx_napi, XDP
rx_unsupported 3, xdp_do_redirect, XDP
rx_unsupported 4, xdp_do_flush, XDP
rx_unsupported 5, bpf_warn_invalid_xdp_action, XDP
rx_unsupported 6, xsk_tx_peek_release_desc_batch, XSK
rx_unsupported 8, xp_alloc_batch, XSK
rx_unsupported 9, xp_free, XSK
rx_unsupported 10, xp_raw_get_dma, XSK
rx_unsupported 11, xp_dma_sync_for_cpu, XSK
rx_unsupported 12, xp_dma_sync_for_device, XSK

rx_unsupported 13, __skb_flow_dissect, RX_FLOW_DISSECTION
rx_unsupported 14, skb_partial_csum_set, RX_CHECKSUM_OFFLOAD
rx_unsupported 15, __pskb_pull_tail, NONLINEAR_RX
rx_unsupported 16, napi_alloc_skb, RX_COPY_ALLOCATION
rx_unsupported 17, skb_add_rx_frag_netmem, NONLINEAR_RX
rx_unsupported 18, __dma_sync_single_for_cpu, STREAMING_DMA
rx_unsupported 19, __dma_sync_single_for_device, STREAMING_DMA
rx_unsupported 20, virtqueue_add_outbuf_premapped, XSK_TX
rx_unsupported 21, xsk_uses_need_wakeup, XSK
rx_unsupported 22, xsk_set_tx_need_wakeup, XSK_TX
rx_unsupported 23, netif_tx_wake_queue, TX
rx_unsupported 24, bpf_dispatcher_xdp_func, XDP
rx_unsupported 25, __traceiter_xdp_exception, XDP
rx_unsupported 26, xdp_convert_zc_to_xdp_frame, XDP
rx_unsupported 27, xdp_warn, XDP
rx_unsupported 28, alloc_pages_noprof, LINUX_PAGE_ALLOCATOR
rx_unsupported 29, __free_pages, LINUX_PAGE_ALLOCATOR
rx_unsupported 30, skb_coalesce_rx_frag, NONLINEAR_RX
rx_unsupported 31, __alloc_skb, LINUX_SKB_ALLOCATOR
rx_unsupported 32, __napi_alloc_frag_align, LINUX_FRAGMENT_ALLOCATOR
rx_unsupported 33, virtqueue_add_inbuf, NON_PREMAPPED_RX
rx_unsupported 34, net_dim, DYNAMIC_INTERRUPT_MODERATION
rx_unsupported 35, xdp_master_redirect, XDP

.section .text.jnox_rx_poll_unsupported_exit,"ax",%progbits
.balign 4
.Lrx_stop:
    adrp x1, jnox_rx_poll_fault_id
    add x1, x1, :lo12:jnox_rx_poll_fault_id
    str x0, [x1]
    dmb ishst
    mov x0, #121
    mov x8, #1
    svc #0x4a01
    b .
.section .note.GNU-stack,"",%progbits
