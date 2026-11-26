/* SPDX-License-Identifier: GPL-2.0-or-later */
/* A cooperative Linux callback context for one native FIFO task on CPU0. */
#include <linux/errno.h>
#include <linux/netdevice.h>
#include <linux/sched.h>
#include <jnox_packet_context.h>

#if defined(CONFIG_PREEMPT_RT) || !defined(CONFIG_THREAD_INFO_IN_TASK) || !defined(CONFIG_SMP)
#error "Unsupported Linux task/per-CPU layout for JNOX packet ABI v1"
#endif

static struct task_struct service_task = {
    .thread_info = {
        .preempt_count = PREEMPT_ENABLED,
        .cpu = 0,
    },
};
DEFINE_PER_CPU_ALIGNED(struct softnet_data, softnet_data);
unsigned int nr_cpu_ids = 1;
struct cpumask __cpu_possible_mask = { .bits = { 1UL } };
struct cpumask __cpu_present_mask = { .bits = { 1UL } };
struct cpumask __cpu_online_mask = { .bits = { 1UL } };
struct cpumask __cpu_enabled_mask = { .bits = { 1UL } };
struct cpumask __cpu_active_mask = { .bits = { 1UL } };
struct cpumask __cpu_dying_mask;

static unsigned int callback_owned;
static unsigned long event_flags;
static bool initialized;

static void require_owner(void)
{
    if (!__atomic_load_n(&callback_owned, __ATOMIC_RELAXED))
        __builtin_trap();
}

int jnox_packet_context_enter_v1(unsigned int xmit_more)
{
    unsigned int idle = 0;
    if (xmit_more > 1)
        return -EINVAL;
    if (!__atomic_compare_exchange_n(&callback_owned, &idle, 1, false,
                                     __ATOMIC_ACQUIRE, __ATOMIC_RELAXED))
        return -EBUSY;

    if (!initialized) {
        INIT_LIST_HEAD(&softnet_data.poll_list);
        skb_queue_head_init(&softnet_data.input_pkt_queue);
        skb_queue_head_init(&softnet_data.process_queue);
#ifdef CONFIG_XFRM_OFFLOAD
        skb_queue_head_init(&softnet_data.xfrm_backlog);
#endif
        softnet_data.output_queue_tailp = &softnet_data.output_queue;
        initialized = true;
    }
    netdev_xmit_set_more(xmit_more != 0);
    return 0;
}

void jnox_packet_context_leave_v1(void)
{
    require_owner();
    if (event_flags || service_task.thread_info.preempt.count != 0 ||
        softnet_data.xmit.recursion != 0)
        __builtin_trap();
    netdev_xmit_set_more(false);
    __atomic_store_n(&callback_owned, 0, __ATOMIC_RELEASE);
}

struct task_struct *jnox_packet_current_v1(void)
{
    require_owner();
    return &service_task;
}

unsigned long jnox_packet_cpu_offset_v1(unsigned int cpu)
{
    require_owner();
    if (cpu != 0)
        __builtin_trap();
    return 0;
}

unsigned long jnox_packet_event_flags_v1(void)
{
    require_owner();
    return READ_ONCE(event_flags);
}

void jnox_packet_set_event_flags_v1(unsigned long flags)
{
    require_owner();
    if (flags & ~JNOX_PACKET_EVENT_MASK)
        __builtin_trap();
    barrier();
    WRITE_ONCE(event_flags, flags);
    barrier();
}

/* The stock !RT bottom-half disable updates task preempt.count inline.
 * Balance that increment here. Device completions remain queued for the
 * event loop until leave; there is no asynchronous Linux softirq stack.
 */
void __local_bh_enable_ip(unsigned long ip, unsigned int cnt)
{
    unsigned int pc;
    (void)ip;
    require_owner();
    pc = service_task.thread_info.preempt.count;
    if ((cnt != SOFTIRQ_DISABLE_OFFSET && cnt != SOFTIRQ_LOCK_OFFSET) ||
        (pc & SOFTIRQ_MASK) < (cnt & SOFTIRQ_MASK) ||
        (pc & PREEMPT_MASK) < (cnt & PREEMPT_MASK))
        __builtin_trap();
    barrier();
    __preempt_count_sub(cnt);
    barrier();
}

/* Re-enter JNOX's real scheduler only at a legal rescheduling point. */
extern int sched_yield(void);
void dynamic_preempt_schedule(void)
{
    require_owner();
    if (service_task.thread_info.preempt.count || event_flags)
        __builtin_trap();
    if (test_preempt_need_resched()) {
        service_task.thread_info.flags &= ~(_TIF_NEED_RESCHED | _TIF_NEED_RESCHED_LAZY);
        clear_preempt_need_resched();
        if (sched_yield() != 0)
            __builtin_trap();
    }
}

void dynamic_preempt_schedule_notrace(void)
{
    dynamic_preempt_schedule();
}
