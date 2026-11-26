/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_PACKET_CONTEXT_H
#define JNOX_PACKET_CONTEXT_H

#if !defined(JNOX_PACKET_CONTEXT_ABI) || JNOX_PACKET_CONTEXT_ABI != 1
#error "This execution-context port requires JNOX_PACKET_CONTEXT_ABI=1"
#endif

/* Exactly one FIFO event-loop thread per service. EL1 may preempt it.
 * Queue/device interrupts enqueue events; they never enter a user callback
 * asynchronously. An accepted entry must be paired with leave on all paths.
 */
int jnox_packet_context_enter_v1(unsigned int xmit_more);
void jnox_packet_context_leave_v1(void);

struct task_struct;
struct task_struct *jnox_packet_current_v1(void);
unsigned long jnox_packet_cpu_offset_v1(unsigned int cpu);
unsigned long jnox_packet_event_flags_v1(void);
void jnox_packet_set_event_flags_v1(unsigned long flags);

#endif
