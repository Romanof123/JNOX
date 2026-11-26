/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef __ASM_IRQFLAGS_H
#define __ASM_IRQFLAGS_H
#include <linux/compiler.h>
#include <linux/types.h>
#include <asm/barrier.h>
#include <asm/ptrace.h>
#include <asm/sysreg.h>
#include <jnox_packet_context.h>

/* Only service-local callback delivery is gated. Physical IRQ state belongs
 * to JNOX; the event loop dispatches a pending event after the callback exits.
 * Save/restore uses the ARM I-bit representation and supports nested guards.
 */
#define JNOX_PACKET_EVENT_MASK (1UL << 7)
static inline unsigned long arch_local_save_flags(void)
{
    return jnox_packet_event_flags_v1();
}
static inline bool arch_irqs_disabled_flags(unsigned long flags)
{
    return (flags & JNOX_PACKET_EVENT_MASK) != 0;
}
static inline bool arch_irqs_disabled(void)
{
    return arch_irqs_disabled_flags(arch_local_save_flags());
}
static inline void arch_local_irq_disable(void)
{
    jnox_packet_set_event_flags_v1(JNOX_PACKET_EVENT_MASK);
}
static inline void arch_local_irq_enable(void)
{
    jnox_packet_set_event_flags_v1(0);
}
static inline unsigned long arch_local_irq_save(void)
{
    unsigned long flags = arch_local_save_flags();
    arch_local_irq_disable();
    return flags;
}
static inline void arch_local_irq_restore(unsigned long flags)
{
    jnox_packet_set_event_flags_v1(flags);
}
#endif
