// SPDX-License-Identifier: GPL-2.0-or-later
/* Single-owner FIFO admission for one Linux RX/TX queue pair. IRQ delivery
 * is serialized with this worker. No other thread executes in these objects. */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <jnox_rx_napi.h>

static struct send_queue sq;
static struct receive_queue rq;
static struct napi_config cfg;
#ifdef JNOX_PACKET_RX_IRQ_ABI
static struct virtqueue *pair_tmp[2];
static struct virtqueue_info info_tmp[2];
static bool ctx_tmp[2];
static bool pair_busy, info_busy, ctx_busy;

#endif
static struct net_device *owner;
static bool sq_alloc, rq_alloc, locked, polling, queued, missed;
#ifdef JNOX_PACKET_RX_IRQ_ABI
bool jnox_rx_napi_pending_v1(void) { return queued; }
#endif
static int current_budget;
static raw_spinlock_t *held_spin;
static struct task_struct *rcu_owner;
static unsigned long long counts[8];
void jnox_rx_napi_stats_v1(unsigned long long out[8])
{
    for (unsigned i = 0; i < ARRAY_SIZE(counts); ++i) out[i] = counts[i];
}
void jnox_rx_napi_event_v1(unsigned mode, unsigned value)
{
    if (mode == 1) ++counts[5];
    if (mode == 2 && value == 1) ++counts[6];
    if (mode == 3) ++counts[7];
}
volatile unsigned int jnox_rx_napi_fault;

static __noreturn void bad(unsigned int reason)
{
    WRITE_ONCE(jnox_rx_napi_fault, reason);
    __builtin_trap();
}

int jnox_rx_napi_prepare_v1(struct net_device *dev)
{
    if (!dev || owner) return -EBUSY;
    owner = dev;
    memset(&cfg, 0, sizeof(cfg));
    dev->napi_config = &cfg;
    INIT_LIST_HEAD(&dev->napi_list);
    mutex_init(&dev->lock);
    return 0;
}

void *jnox_rx_queue_calloc_v1(size_t count, size_t size, gfp_t flags)
{
    (void)flags;
#ifdef JNOX_PACKET_RX_IRQ_ABI
    if (owner && count == 2) {
        if (size == sizeof(*pair_tmp) && !pair_busy) {
            pair_busy = true;
            return memset(pair_tmp, 0, sizeof(pair_tmp));
        }
        if (size == sizeof(*info_tmp) && !info_busy) {
            info_busy = true;
            return memset(info_tmp, 0, sizeof(info_tmp));
        }
        if (size == sizeof(*ctx_tmp) && !ctx_busy) {
            ctx_busy = true;
            return memset(ctx_tmp, 0, sizeof(ctx_tmp));
        }
        return NULL;
    }
#endif
    if (!owner || count != 1) return NULL;
    if (size == sizeof(sq) && !sq_alloc) {
        memset(&sq, 0, sizeof(sq));
        sq_alloc = true;
        return &sq;
    }
    if (size == sizeof(rq) && !rq_alloc) {
        memset(&rq, 0, sizeof(rq));
        rq_alloc = true;
        return &rq;
    }
    return NULL;
}

void kfree(const void *ptr)
{
    if (!ptr) return;
#ifdef JNOX_PACKET_LIFECYCLE_ABI
    extern bool jnox_lifecycle_common_free_v1(const void *);
    if (jnox_lifecycle_common_free_v1(ptr)) return;
#endif
#ifdef JNOX_PACKET_RX_IRQ_ABI
    if (ptr == pair_tmp && pair_busy) { pair_busy = false; return; }
    if (ptr == info_tmp && info_busy) { info_busy = false; return; }
    if (ptr == ctx_tmp && ctx_busy) { ctx_busy = false; return; }
#endif
    #ifdef JNOX_PACKET_LIFECYCLE_ABI
    if (ptr == &sq && list_empty(&sq.napi.dev_list)) sq.napi.dev = NULL;
    if (ptr == &rq && list_empty(&rq.napi.dev_list)) rq.napi.dev = NULL;
#endif
    if (ptr == &sq && sq_alloc && !sq.napi.dev) sq_alloc = false;
    else if (ptr == &rq && rq_alloc && !rq.napi.dev) rq_alloc = false;
    else bad(1);
}

int jnox_rx_queue_large_sg_v1(struct scatterlist *sg, unsigned int count)
{
    return (rq_alloc && sg == rq.sg && count == ARRAY_SIZE(rq.sg)) ||
           (sq_alloc && sg == sq.sg && count == ARRAY_SIZE(sq.sg));
}

void __mutex_init(struct mutex *m, const char *name, struct lock_class_key *key)
{
    (void)name;
    (void)key;
    if (!owner || (m != &owner->lock && (!rq_alloc || m != &rq.dim_lock))) bad(2);
    memset(m, 0, sizeof(*m));
    INIT_LIST_HEAD(&m->wait_list);
}

void mutex_lock(struct mutex *m)
{
    if (!owner || m != &owner->lock || locked) bad(3);
    atomic_long_set(&m->owner, (long)jnox_packet_current_v1());
    locked = true;
}

void mutex_unlock(struct mutex *m)
{
    if (!owner || m != &owner->lock || !locked) bad(4);
    atomic_long_set(&m->owner, 0);
    locked = false;
}

void timer_init_key(struct timer_list *t, void (*fn)(struct timer_list *), unsigned int flags,
                    const char *name, struct lock_class_key *key)
{
    (void)name;
    (void)key;
    if (!owner || t != &((struct virtnet_info *)netdev_priv(owner))->refill.timer ||
        fn != delayed_work_timer_fn || !(flags & TIMER_IRQSAFE)) bad(5);
    memset(t, 0, sizeof(*t));
    t->function = fn;
    t->flags = flags;
}

void delayed_work_timer_fn(struct timer_list *timer)
{
    (void)timer;
    /* Refill is synchronous in the admitted original polling branch. */
    bad(6);
}

void netif_napi_add_weight_locked(struct net_device *dev, struct napi_struct *n,
                                  int (*poll)(struct napi_struct *, int), int weight)
{
    bool is_rx = rq_alloc && n == &rq.napi;
    bool is_tx = sq_alloc && n == &sq.napi;
    if (!owner || dev != owner || !locked || (!is_rx && !is_tx) ||
        !poll || n->dev || weight < 0 || weight > NAPI_POLL_WEIGHT) bad(7);
    if (is_rx && (n->index != 0 || n->config != &cfg)) bad(8);
    ++counts[is_rx ? 0 : 1];
    n->dev = dev;
    n->poll = poll;
    n->weight = weight;
    n->napi_id = is_rx ? 1 : 2;
    if (is_rx) cfg.napi_id = n->napi_id;
    INIT_LIST_HEAD(&n->poll_list);
    /* Single-owner insertion without Linux's debug-list globals. */
    n->dev_list.next = &dev->napi_list;
    n->dev_list.prev = dev->napi_list.prev;
    n->dev_list.prev->next = &n->dev_list;
    dev->napi_list.prev = &n->dev_list;
    n->state |= BIT(NAPI_STATE_DISABLED);
}

bool napi_schedule_prep(struct napi_struct *n)
{
    if (!owner || n != &rq.napi || !n->dev || (n->state & BIT(NAPI_STATE_DISABLED))) return false;
    if (n->state & BIT(NAPI_STATE_SCHED)) {
        missed = true;
        n->state |= BIT(NAPI_STATE_MISSED);
        return false;
    }
    n->state |= BIT(NAPI_STATE_SCHED);
    return true;
}

void __napi_schedule(struct napi_struct *n)
{
    if (!owner || n != &rq.napi || !(n->state & BIT(NAPI_STATE_SCHED))) bad(9);
    queued = true;
}

bool napi_complete_done(struct napi_struct *n, int done)
{
    if (!polling || n != &rq.napi || !(n->state & BIT(NAPI_STATE_SCHED)) ||
        done < 0 || done >= current_budget) bad(10);
    if (missed) {
        missed = false;
        n->state &= ~BIT(NAPI_STATE_MISSED);
        queued = true;
        return false;
    }
    ++counts[4];
    n->state &= ~BIT(NAPI_STATE_SCHED);
    queued = false;
    return true;
}

int jnox_rx_napi_poll_v1(struct napi_struct *n, int budget)
{
    if (!owner || !rq_alloc || n != &rq.napi || n->dev != owner || !n->poll ||
        budget <= 0 || budget > n->weight || polling) return -EINVAL;
#ifndef JNOX_PACKET_RX_IRQ_ABI
    n->state &= ~BIT(NAPI_STATE_DISABLED);
    if (!queued && napi_schedule_prep(n)) __napi_schedule(n);
#endif
    if (!queued) return -EAGAIN;
    queued = false;
    polling = true;
    current_budget = budget;
    ++counts[2];
    int work = n->poll(n, budget);
    polling = false;
    if (work < 0 || work > budget) return -EPROTO;
    if (work == budget) {
        ++counts[3];
        if (!(n->state & BIT(NAPI_STATE_SCHED))) return -EPROTO;
        queued = true;
    }
    return work;
}

static void remove_napi(struct napi_struct *n)
{
    if (!n->dev) return;
    n->dev_list.next->prev = n->dev_list.prev;
    n->dev_list.prev->next = n->dev_list.next;
    INIT_LIST_HEAD(&n->dev_list);
    n->state = BIT(NAPI_STATE_DISABLED);
    n->dev = NULL;
    n->poll = NULL;
}

int jnox_rx_napi_release_v1(struct net_device *dev)
{
    if (!dev || dev != owner || polling || locked || held_spin || rcu_owner) return -EBUSY;
    remove_napi(&rq.napi);
    remove_napi(&sq.napi);
    if (rq_alloc) kfree(&rq);
    if (sq_alloc) kfree(&sq);
    struct virtnet_info *vi = netdev_priv(dev);
    vi->rq = NULL;
    vi->sq = NULL;
    dev->napi_config = NULL;
    queued = missed = false;
#ifdef JNOX_PACKET_LIFECYCLE_ABI
    extern void jnox_lifecycle_clear_work_v1(void);
    jnox_lifecycle_clear_work_v1();
#endif
    owner = NULL;
    return 0;
}

/* Only this FIFO task operates on the admitted TX queue and associated
 * netdev. The atomic state still provides acquire/release ordering. A
 * recursive blocking lock cannot progress on this worker. */
static bool known_spin(raw_spinlock_t *lock)
{
    #ifdef JNOX_PACKET_LIFECYCLE_ABI
    if (owner && lock == &((struct virtnet_info *)netdev_priv(owner))->refill_lock.rlock) return true;
#endif
    return owner && owner->_tx && lock == &owner->_tx[0]._xmit_lock.rlock;
}

int _raw_spin_trylock(raw_spinlock_t *lock)
{
    if (!known_spin(lock)) bad(11);
    int unlocked = 0;
    if (!__atomic_compare_exchange_n(&lock->raw_lock.val.counter, &unlocked, 1,
                                     false, __ATOMIC_ACQUIRE, __ATOMIC_RELAXED))
        return 0;
    if (held_spin) bad(12);
    held_spin = lock;
    return 1;
}

void _raw_spin_lock(raw_spinlock_t *lock)
{
    if (!_raw_spin_trylock(lock)) bad(13);
}

void _raw_spin_unlock(raw_spinlock_t *lock)
{
    if (!known_spin(lock) || lock != held_spin || READ_ONCE(lock->raw_lock.val.counter) != 1)
        bad(14);
    held_spin = NULL;
    __atomic_store_n(&lock->raw_lock.val.counter, 0, __ATOMIC_RELEASE);
}

/* Readers execute in the single registered worker. Unregister/reclamation
 * refuses to run while any nested read-side critical section is active. */
void __rcu_read_lock(void)
{
    struct task_struct *t = jnox_packet_current_v1();
    if (!owner || !t || (rcu_owner && rcu_owner != t) ||
        t->rcu_read_lock_nesting < 0 || t->rcu_read_lock_nesting == INT_MAX)
        bad(15);
    rcu_owner = t;
    WRITE_ONCE(t->rcu_read_lock_nesting, t->rcu_read_lock_nesting + 1);
    barrier();
}

void __rcu_read_unlock(void)
{
    struct task_struct *t = jnox_packet_current_v1();
    if (!t || t != rcu_owner || t->rcu_read_lock_nesting <= 0) bad(16);
    barrier();
    WRITE_ONCE(t->rcu_read_lock_nesting, t->rcu_read_lock_nesting - 1);
    if (!t->rcu_read_lock_nesting) rcu_owner = NULL;
}

bool __list_del_entry_valid_or_report(struct list_head *entry)
{
    if (!entry || !entry->next || !entry->prev || entry->next == LIST_POISON1 ||
        entry->prev == LIST_POISON2 || entry->next->prev != entry || entry->prev->next != entry)
        bad(17);
    return true;
}

void napi_disable(struct napi_struct *n)
{
    if (!owner || (n != &rq.napi && n != &sq.napi) || n->dev != owner ||
        (n->state & BIT(NAPI_STATE_DISABLED)) || (n == &rq.napi && polling)) bad(18);
    n->state |= BIT(NAPI_STATE_DISABLED);
    n->state &= ~(BIT(NAPI_STATE_SCHED) | BIT(NAPI_STATE_MISSED));
    if (n == &rq.napi) queued = missed = false;
}

void napi_enable(struct napi_struct *n)
{
        if (!owner ||
#ifdef JNOX_PACKET_LIFECYCLE_ABI
        (n != &rq.napi && n != &sq.napi) ||
#else
        n != &rq.napi ||
#endif
        n->dev != owner || !(n->state & BIT(NAPI_STATE_DISABLED)))
        bad(19);
    n->state &= ~BIT(NAPI_STATE_DISABLED);
}

/* Linux exposes workqueue_struct as opaque. This native endpoint admits
 * synchronous poll-time refill only. Any request for deferred refill is a
 * recorded service failure, never a successful queue submission. */
struct workqueue_struct { unsigned int rejected; };
static struct workqueue_struct refill_queue;
struct workqueue_struct *system_percpu_wq = &refill_queue;

#ifndef JNOX_PACKET_LIFECYCLE_ABI
bool queue_delayed_work_on(int cpu, struct workqueue_struct *wq,
                           struct delayed_work *work, unsigned long delay)
{
    (void)delay;
    if (!owner || wq != system_percpu_wq ||
        work != &((struct virtnet_info *)netdev_priv(owner))->refill ||
        (cpu != WORK_CPU_UNBOUND && cpu != 0)) bad(20);
    ++refill_queue.rejected;
    bad(21);
}
#endif
#ifdef JNOX_PACKET_RX_IRQ_ABI
bool jnox_rx_equal_name_v1(const char *a, const char *b)
{
    while (*a && *a == *b) {
        ++a;
        ++b;
    }
    return *a == *b;
}

int jnox_rx_queue_name_v1(char *dst, const char *fmt, unsigned int index)
{
    if (index == 0 && rq_alloc && dst == rq.name && jnox_rx_equal_name_v1(fmt, "input.%u")) {
        memcpy(dst, "input.0", sizeof("input.0"));
        return sizeof("input.0") - 1;
    }
    if (index == 0 && sq_alloc && dst == sq.name && jnox_rx_equal_name_v1(fmt, "output.%u")) {
        memcpy(dst, "output.0", sizeof("output.0"));
        return sizeof("output.0") - 1;
    }
    bad(22);
}
#endif

#ifdef JNOX_PACKET_LIFECYCLE_ABI
void _raw_spin_lock_bh(raw_spinlock_t *lock) { _raw_spin_lock(lock); }
void _raw_spin_unlock_bh(raw_spinlock_t *lock) { _raw_spin_unlock(lock); }

void __netif_napi_del_locked(struct napi_struct *n)
{
    if (!owner || !locked || (n != &sq.napi && n != &rq.napi) || n->dev != owner ||
        !(n->state & BIT(NAPI_STATE_DISABLED)) || list_empty(&n->dev_list)) bad(23);
    remove_napi(n);
    /* Header __netif_napi_del unlocks napi->dev after this call. */
    n->dev = owner;
}
void synchronize_net(void)
{
    if (rcu_owner || polling) bad(24);
    barrier();
}

struct work_entry {
    struct work_struct *work;
    unsigned disabled;
    bool pending, executing;
    u64 due;
};
static struct work_entry works[3];
static struct work_entry *work_entry(struct work_struct *w)
{
    if (!owner || !w || !w->func) bad(25);
    struct virtnet_info *vi = netdev_priv(owner);
    unsigned i;
    if (w == &vi->config_work) i = 0;
    else if (w == &vi->rx_mode_work) i = 1;
    else if (w == &vi->refill.work) i = 2;
    else bad(26);
    works[i].work = w;
    return &works[i];
}
static u64 clock_ticks(void)
{
    u64 ticks;
    asm volatile("isb; mrs %0,cntvct_el0" : "=r"(ticks));
    return ticks;
}
static bool queue_entry(int cpu, struct workqueue_struct *q, struct work_struct *w, u64 due)
{
    if ((cpu != WORK_CPU_UNBOUND && cpu != 0) || q != system_percpu_wq) bad(27);
    struct work_entry *e = work_entry(w);
    if (e->disabled || e->pending) return false;
    e->due = due;
    e->pending = true;
    atomic_long_or(WORK_STRUCT_PENDING, &w->data);
    return true;
}
bool queue_work_on(int cpu, struct workqueue_struct *q, struct work_struct *w)
{
    return queue_entry(cpu,q,w,0);
}
bool queue_delayed_work_on(int cpu, struct workqueue_struct *q, struct delayed_work *w,
                           unsigned long delay)
{
    u64 freq;
    asm volatile("mrs %0,cntfrq_el0" : "=r"(freq));
    if (!freq || delay > MAX_JIFFY_OFFSET || delay > (~0ULL / freq)) bad(28);
    return queue_entry(cpu,q,&w->work,clock_ticks() + delay * freq / HZ);
}
static bool cancel_entry(struct work_entry *e)
{
    bool pending = e->pending;
    e->pending = false;
    atomic_long_andnot(WORK_STRUCT_PENDING, &e->work->data);
    return pending;
}
bool cancel_work_sync(struct work_struct *w)
{
    struct work_entry *e = work_entry(w);
    if (e->executing) bad(29);
    return cancel_entry(e);
}
bool cancel_delayed_work_sync(struct delayed_work *w) { return cancel_work_sync(&w->work); }
bool disable_work(struct work_struct *w)
{
    struct work_entry *e = work_entry(w);
    ++e->disabled;
    return cancel_entry(e);
}
bool disable_work_sync(struct work_struct *w)
{
    if (work_entry(w)->executing) bad(30);
    return disable_work(w);
}
bool enable_work(struct work_struct *w)
{
    struct work_entry *e = work_entry(w);
    if (!e->disabled) bad(31);
    return --e->disabled == 0;
}
bool flush_work(struct work_struct *w)
{
    struct work_entry *e = work_entry(w);
    if (e->executing) bad(32);
    if (!cancel_entry(e)) return false;
    e->executing = true;
    w->func(w);
    e->executing = false;
    return true;
}
void jnox_lifecycle_run_work_v1(void)
{
    for (unsigned i = 0; i < ARRAY_SIZE(works); ++i)
        if (works[i].pending && (s64)(clock_ticks() - works[i].due) >= 0)
            flush_work(works[i].work);
}
void jnox_lifecycle_clear_work_v1(void)
{
    for (unsigned i = 0; i < ARRAY_SIZE(works); ++i)
        if (works[i].pending || works[i].executing) bad(33);
    memset(works, 0, sizeof(works));
}
#endif