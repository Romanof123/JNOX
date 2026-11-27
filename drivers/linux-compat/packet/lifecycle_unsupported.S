// SPDX-License-Identifier: GPL-2.0-or-later
// Stable fault identifiers for excluded upstream operations in the lifecycle image.
#ifdef JNOX_PACKET_LIFECYCLE_ABI

.section .bss.jnox_lifecycle_fault_id,"aw",%nobits
.balign 8
.global jnox_lifecycle_fault_id
.type jnox_lifecycle_fault_id,%object
jnox_lifecycle_fault_id:
    .zero 8
.size jnox_lifecycle_fault_id,8

.macro excluded id, name
.section .text.\name,"ax",%progbits
.balign 4
.global \name
.type \name,%function
\name:
    mov x0, #\id
    b .Llifecycle_fault
.size \name,.-\name
.endm

// Userspace does not expose ethtool queries, link settings or ring resize.
excluded 1, ethtool_op_get_link
excluded 2, ethtool_op_get_ts_info
excluded 3, ethtool_sprintf
excluded 4, ethtool_virtdev_set_link_ksettings
excluded 5, netdev_stat_queue_sum
excluded 6, virtqueue_resize

// The image has one queue pair and excludes RSS and control virtqueues.
excluded 7, netdev_rss_key_fill
excluded 8, virtqueue_add_sgs
// ID 9 is reserved for the supported virtqueue adapter.

// No BPF program or XSK pool can be attached by this interface.
excluded 10, bpf_prog_add
excluded 11, bpf_prog_sub
excluded 12, bpf_prog_put
excluded 13, xp_set_rxq_info
excluded 14, xp_dma_map
excluded 15, xp_dma_unmap
// ID 16 is reserved for the supported virtqueue DMA device lookup.
excluded 17, virtqueue_reset
excluded 18, xdp_features_set_redirect_target
excluded 19, xdp_features_clear_redirect_target

// Suspend/restore and STANDBY negotiation are outside this profile.
// IDs 20 and 21 are reserved for the netdevice adapter.

excluded 22, net_failover_create
// ID 23 reports a non-NULL failover cleanup from the typed runtime.
excluded 23, jnox_lifecycle_nonnull_failover_destroy
// Control commands and fallback random MAC are excluded by the feature mask.
excluded 24, sg_init_one
excluded 25, get_random_bytes
// The only linked snprintf caller formats the standby physical port name.
excluded 26, snprintf

.section .text.jnox_lifecycle_unsupported_exit,"ax",%progbits
.balign 4
.Llifecycle_fault:
    adrp x1, jnox_lifecycle_fault_id
    add x1, x1, :lo12:jnox_lifecycle_fault_id
    str x0, [x1]
    dmb ishst
    mov x0, #122
    mov x8, #1
    svc #0x4a01
    brk #0
#endif
.section .note.GNU-stack,"",%progbits
