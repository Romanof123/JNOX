/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef __ASM_PERCPU_H
#define __ASM_PERCPU_H

#include <linux/preempt.h>
#include <asm/cmpxchg.h>
#include <jnox_packet_context.h>

/* CPU0's storage lives at each actual writable linked symbol. The offset
 * Linux adds to a per-CPU symbol is therefore zero, not the section's VA.
 * Explicit access to another CPU is outside this single-CPU service ABI.
 * The generic header skips its Linux allocator's offset array when this
 * marker is present. A directly indexed Linux allocator array is unsupported.
 */
#define __per_cpu_offset jnox_packet_no_linux_percpu_allocator
#define per_cpu_offset(cpu) jnox_packet_cpu_offset_v1(cpu)
#define __my_cpu_offset jnox_packet_cpu_offset_v1(0)
#include <asm-generic/percpu.h>

#endif
