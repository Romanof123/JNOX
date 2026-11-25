#include <errno.h>
#include <jnox/native_abi.h>
#include <pthread.h>
#include <sched.h>
#include "../native-edf/clock.h"

enum { BASE, LOW, MEDIUM, HIGH };
enum { LARGE = 524288, SMALL = 8 };
enum scenario { CHAIN, TIMEOUT, PROTECT, RANK, FIFO12, FIFO21, CYCLE, RR, FIFO };
struct product { uint64_t blocks, sum, squares; };
struct product products[4];
unsigned char ramp[256];
pthread_mutex_t x, y, z;
pthread_t handles[4], retired_worker;
void *joined[4];
unsigned stages[4], transcript[16], transcript_count, failure_line;
int timeout_result, busy_result, wrong_user_result, cycle_result, stale_result, stale_mutex_result;
uint64_t release_ns, timeout_ns;
static enum scenario mode;
extern void reduce_ramps(const unsigned char *, struct product *, uint64_t);
extern _Noreturn void client_abort(void);

#define CHECK(expr) do { if (!(expr)) { failure_line = __LINE__; client_abort(); } } while (0)

static void stage(unsigned id, unsigned value) { __atomic_store_n(&stages[id], value, __ATOMIC_RELEASE); }
static unsigned reached(unsigned id) { return __atomic_load_n(&stages[id], __ATOMIC_ACQUIRE); }

static void note(unsigned id)
{
    unsigned i = __atomic_fetch_add(&transcript_count, 1, __ATOMIC_RELAXED);
    CHECK(i < 16);
    transcript[i] = id;
}

static void work(unsigned id, unsigned n)
{
    reduce_ramps(ramp, &products[id], n);
}

static void verify(unsigned id, unsigned n)
{
    CHECK(products[id].blocks == n && products[id].sum == (uint64_t)n * 32640
          && products[id].squares == (uint64_t)n * 5559680);
}

static void init_lock(pthread_mutex_t *m, int protocol, unsigned users, int ceiling)
{
    pthread_mutexattr_t a;
    CHECK(pthread_mutexattr_init(&a) == 0);
    CHECK(pthread_mutexattr_setprotocol(&a, protocol) == 0);
    CHECK(jnox_mutexattr_setusers(&a, users) == 0);
    if (ceiling) CHECK(pthread_mutexattr_setprioceiling(&a, ceiling) == 0);
    CHECK(pthread_mutex_init(m, &a) == 0);
    CHECK(pthread_mutexattr_destroy(&a) == 0);
}

static void start(unsigned id, int priority, int policy, void *(*fn)(void *))
{
    pthread_attr_t a;
    struct sched_param param = { .sched_priority = priority };
    CHECK(pthread_attr_init(&a) == 0);
    CHECK(pthread_attr_setinheritsched(&a, PTHREAD_EXPLICIT_SCHED) == 0);
    CHECK(pthread_attr_setschedpolicy(&a, policy) == 0);
    CHECK(pthread_attr_setschedparam(&a, &param) == 0);
    CHECK(pthread_create(&handles[id], &a, fn, &products[id]) == 0);
    CHECK(pthread_attr_destroy(&a) == 0);
}

static void collect(unsigned id)
{
    CHECK(pthread_join(handles[id], &joined[id]) == 0 && joined[id] == &products[id]);
}

static void clock_plan(void)
{
    uint64_t now;
    CHECK(read_ns(&now) == 0);
    release_ns = now + 40000000;
    timeout_ns = release_ns + 20000000;
}

static void before_release(void)
{
    uint64_t now;
    CHECK(read_ns(&now) == 0 && now < release_ns);
}

static void *low(void *arg)
{
    CHECK(pthread_mutex_lock(&x) == 0);
    stage(LOW, 1);
    CHECK(pthread_mutex_lock(&y) == 0);
    verify(BASE, mode == CYCLE ? SMALL : LARGE);
    work(LOW, SMALL);
    note(LOW);
    CHECK(pthread_mutex_unlock(&y) == 0);
    CHECK(pthread_mutex_unlock(&x) == 0);
    return arg;
}

static void *medium(void *arg)
{
    CHECK(sleep_to(release_ns) == 0);
    work(MEDIUM, LARGE);
    stage(MEDIUM, 1);
    return arg;
}

static void *high(void *arg)
{
    CHECK(sleep_to(release_ns) == 0);
    stage(HIGH, 1);
    if (mode == TIMEOUT) {
        struct timespec at = { (time_t)(timeout_ns / 1000000000),
                               (long)(timeout_ns % 1000000000) };
        timeout_result = pthread_mutex_clocklock(&x, CLOCK_MONOTONIC, &at);
        CHECK(timeout_result == ETIMEDOUT);
    } else {
        CHECK(pthread_mutex_lock(&x) == 0);
        verify(LOW, SMALL);
        note(HIGH);
        CHECK(pthread_mutex_unlock(&x) == 0);
    }
    work(HIGH, SMALL);
    stage(HIGH, 2);
    return arg;
}

static void chain(void)
{
    init_lock(&x, PTHREAD_PRIO_INHERIT, 15, 0);
    init_lock(&y, PTHREAD_PRIO_INHERIT, 15, 0);
    CHECK(pthread_mutex_lock(&y) == 0);
    start(LOW, 2, SCHED_FIFO, low);
    CHECK(reached(LOW) == 1);
    if (mode == CYCLE) {
        cycle_result = pthread_mutex_lock(&x);
        CHECK(cycle_result == EDEADLK);
    } else {
        clock_plan();
        start(MEDIUM, 3, SCHED_FIFO, medium);
        start(HIGH, 4, SCHED_FIFO, high);
        before_release();
        if (mode == TIMEOUT) {
            CHECK(sleep_to(timeout_ns + 20000000) == 0);
            CHECK(reached(HIGH) == 2);
        }
    }
    work(BASE, mode == CYCLE ? SMALL : LARGE);
    if (mode == CHAIN) CHECK(reached(HIGH) == 1 && !reached(MEDIUM));
    note(BASE);
    CHECK(pthread_mutex_unlock(&y) == 0);
    collect(LOW);
    if (mode != CYCLE) {
        collect(MEDIUM);
        collect(HIGH);
        verify(MEDIUM, LARGE);
        verify(HIGH, SMALL);
    }
    CHECK(transcript_count == (mode == CHAIN ? 3u : 2u));
    CHECK(transcript[0] == BASE && transcript[1] == LOW);
    if (mode == CHAIN) CHECK(transcript[2] == HIGH);
    CHECK(pthread_mutex_destroy(&x) == 0 && pthread_mutex_destroy(&y) == 0);
}

static void *outsider(void *arg)
{
    wrong_user_result = pthread_mutex_trylock(&x);
    CHECK(wrong_user_result == EPERM);
    verify(BASE, SMALL);
    work(MEDIUM, SMALL);
    note(MEDIUM);
    return arg;
}

static void protect(void)
{
    pthread_mutexattr_t invalid;
    CHECK(pthread_mutexattr_init(&invalid) == 0);
    CHECK(pthread_mutexattr_setprotocol(&invalid, PTHREAD_PRIO_PROTECT) == 0);
    CHECK(jnox_mutexattr_setusers(&invalid, 0) == EINVAL);
    CHECK(jnox_mutexattr_setusers(&invalid, 16) == EINVAL);
    CHECK(jnox_mutexattr_setusers(&invalid, 9) == 0);
    CHECK(pthread_mutexattr_setprioceiling(&invalid, 2) == 0);
    CHECK(pthread_mutex_init(&x, &invalid) == EINVAL);
    CHECK(pthread_mutexattr_destroy(&invalid) == 0);
    init_lock(&x, PTHREAD_PRIO_PROTECT, 3, 2);
    init_lock(&y, PTHREAD_PRIO_PROTECT, 15, 4);
    init_lock(&z, PTHREAD_PRIO_INHERIT, 15, 0);
    CHECK(pthread_mutex_lock(&x) == 0 && pthread_mutex_lock(&y) == 0);
    CHECK(pthread_mutex_lock(&z) == EAGAIN);
    CHECK(pthread_mutex_unlock(&x) == EINVAL);
    busy_result = pthread_mutex_destroy(&y);
    CHECK(busy_result == EBUSY);
    work(BASE, SMALL);
    note(BASE);
    start(MEDIUM, 3, SCHED_FIFO, outsider);
    CHECK(pthread_mutex_unlock(&y) == 0);
    CHECK(pthread_mutex_unlock(&x) == 0);
    collect(MEDIUM);
    verify(MEDIUM, SMALL);
    CHECK(transcript_count == 2 && transcript[0] == BASE && transcript[1] == MEDIUM);
    CHECK(pthread_mutex_destroy(&z) == 0 && pthread_mutex_destroy(&y) == 0
          && pthread_mutex_destroy(&x) == 0);
}

static void *waiter(void *arg)
{
    unsigned id = (unsigned)((struct product *)arg - products);
    unsigned first = mode == FIFO21 ? MEDIUM : LOW;
    CHECK(sleep_to(release_ns + (id == first ? 0 : 20000000)) == 0);
    stage(id, 1);
    CHECK(pthread_mutex_lock(&x) == 0);
    verify(BASE, SMALL);
    work(id, SMALL);
    note(id);
    CHECK(pthread_mutex_unlock(&x) == 0);
    return arg;
}

static void *reuse(void *arg)
{
    CHECK(pthread_mutex_unlock(&x) == EPERM);
    return arg;
}

static void ordered(void)
{
    init_lock(&x, PTHREAD_PRIO_INHERIT, 15, 0);
    CHECK(pthread_mutex_lock(&x) == 0);
    clock_plan();
    start(LOW, 2, SCHED_FIFO, waiter);
    start(MEDIUM, mode == RANK ? 3 : 2, SCHED_FIFO, waiter);
    before_release();
    CHECK(sleep_to(release_ns + 50000000) == 0);
    CHECK(reached(LOW) && reached(MEDIUM));
    work(BASE, SMALL);
    note(BASE);
    CHECK(pthread_mutex_unlock(&x) == 0);
    collect(LOW);
    collect(MEDIUM);
    verify(LOW, SMALL);
    verify(MEDIUM, SMALL);
    unsigned winner = mode == FIFO12 ? LOW : MEDIUM;
    CHECK(transcript_count == 3 && transcript[1] == winner && transcript[2] == 3 - winner);
    retired_worker = handles[LOW];
    CHECK(pthread_mutex_lock(&x) == 0);
    start(LOW, 2, SCHED_FIFO, reuse);
    collect(LOW);
    CHECK(pthread_mutex_unlock(&x) == 0);
    stale_result = pthread_join(retired_worker, 0);
    CHECK(stale_result == ESRCH && handles[LOW] != retired_worker);
    uint64_t old_token = x.opaque;
    CHECK(pthread_mutex_destroy(&x) == 0);
    init_lock(&x, PTHREAD_PRIO_INHERIT, 15, 0);
    // Raw ABI control at the same live, reinitialized object address.
    register uint64_t address __asm__("x0") = (uint64_t)&x;
    register uint64_t token __asm__("x1") = old_token;
    register uint64_t op __asm__("x8") = JNOX_CALL_MUTEX_TRYLOCK;
    __asm__ volatile("svc #0x4a01" : "+r"(address), "+r"(token) : "r"(op) : "memory", "cc");
    stale_mutex_result = (int)address;
    CHECK(stale_mutex_result == -EINVAL);
    CHECK(pthread_mutex_trylock(&x) == 0 && pthread_mutex_unlock(&x) == 0);
    CHECK(pthread_mutex_destroy(&x) == 0);
}

static void *compete(void *arg)
{
    unsigned id = (unsigned)((struct product *)arg - products);
    CHECK(sleep_to(release_ns + (id == HIGH ? 25000000 : 0)) == 0);
    work(id, id == HIGH ? SMALL : LARGE);
    note(id);
    return arg;
}

static void competition(void)
{
    clock_plan();
    int policy = mode == RR ? SCHED_RR : SCHED_FIFO;
    start(LOW, 2, policy, compete);
    start(MEDIUM, 2, policy, compete);
    start(HIGH, 4, SCHED_FIFO, compete);
    before_release();
    for (unsigned i = LOW; i <= HIGH; ++i) {
        collect(i);
        verify(i, i == HIGH ? SMALL : LARGE);
    }
}

static int equals(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

int main(int argc, char **argv)
{
    static const char *const names[] = {
        "chain","timeout","protect","rank","fifo12","fifo21","cycle","rr","fifo"
    };
    if (argc != 2) return 98;
    for (mode = CHAIN; mode <= FIFO; ++mode)
        if (equals(argv[1], names[mode])) break;
    if (mode > FIFO) return 98;
    for (unsigned i = 0; i < 256; ++i) ramp[i] = (unsigned char)i;
    if (mode == PROTECT) protect();
    else if (mode == RANK || mode == FIFO12 || mode == FIFO21) ordered();
    else if (mode == RR || mode == FIFO) competition();
    else chain();
    stage(BASE, 3);
    return 0;
}
