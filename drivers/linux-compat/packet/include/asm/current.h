/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef __ASM_CURRENT_H
#define __ASM_CURRENT_H
#ifndef __ASSEMBLY__
#include <linux/compiler.h>
#include <jnox_packet_context.h>

static __always_inline struct task_struct *get_current(void)
{
    return jnox_packet_current_v1();
}
#define current get_current()
#endif
#endif
