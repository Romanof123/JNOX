#define _GNU_SOURCE
#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include "sdk/native-aarch64/include/jnox/clock_pair.h"
#include "sdk/native-aarch64/include/jnox/native_abi.h"

_Static_assert(sizeof(long) == 8 && CLOCK_PROCESS_CPUTIME_ID == 2 && CLOCK_MONOTONIC == 1,
               "control host must support the frozen clock ABI");
static uint64_t *const page = (void *)0x01400000UL;
static long discovery;
static uint64_t tick;
static unsigned changes;
static long statuses[2];
static struct timespec times[2];
static _Thread_local char calls[64];
static _Thread_local unsigned ncalls;
static pthread_barrier_t start;

static void record(char op)
{
    assert(ncalls + 1 < sizeof calls);
    calls[ncalls++] = op;
    calls[ncalls] = 0;
}

static long raw_service(long op, long first, long second, long third, long fourth)
{
    assert(!third && !fourth);
    if (op == JNOX_CALL_CLOCK_PAGE) {
        assert(!first && !second);
        record('D');
        return discovery;
    }
    assert(op == JNOX_CALL_CLOCK_GETTIME && (first == 1 || first == 2) && second);
    unsigned i = first == 2 ? 0 : 1;
    record(i ? 'M' : 'P');
    *(struct timespec *)second = times[i];
    return statuses[i];
}

static uint64_t control_counter(void)
{
    record('C');
    if (changes) {
        --changes;
        page[0] += 2;
        page[6] += 10;
    }
    return tick;
}

/* Observe the sequence load without changing its value or atomic order. */
#define __atomic_load_n(p, order) \
    ((void)((const void *)(p) == (const void *)page ? (record('S'), 0) : 0), \
     __atomic_load_n((p), (order)))
#include "sampler.inc"
#undef __atomic_load_n

static void fresh(void)
{
    const uint64_t payload[] = {2, 1, 0, 0, 3, 1, 1, 3};
    memcpy(page, payload, sizeof payload);
    tick = 5;
    changes = 0;
    pair_identity = pair_discovery = 0; /* pristine generation BSS */
    discovery = (long)page;
    times[0] = (struct timespec){0, 17};
    times[1] = (struct timespec){1, 19};
    statuses[0] = statuses[1] = 0;
}

static void expect(const char *events, unsigned backend, unsigned error,
                   uint64_t mono, uint64_t cpu)
{
    ncalls = 0;
    calls[0] = 0;
    errno = EBUSY;
    struct jnox_clock_pair got = jnox_clock_sample();
    if (strcmp(calls, events) || got.backend != backend || got.error != error ||
        got.monotonic_ns != mono || got.process_cpu_ns != cpu || errno != EBUSY) {
        fprintf(stderr, "expected %s/%u/%u; got %s/%u/%u mono=%llu cpu=%llu\n",
                events, backend, error, calls, got.backend, got.error,
                (unsigned long long)got.monotonic_ns, (unsigned long long)got.process_cpu_ns);
        assert(0);
    }
}

static void fast_and_retry(void)
{
    fresh();
    expect("DSCS", 1, 0, 1333333333, 1000000000);
    expect("SCS", 1, 0, 1333333333, 1000000000);
    page[3] = 1;
    expect("SCSSCSPM", 2, 0, 1000000019, 17);
    fresh();
    page[3] = 1;
    expect("DSCS", 1, 0, 1333333333, 1000000000);
    page[2] = 1;
    expect("SCSSCSPM", 2, 0, 1000000019, 17);
    fresh();
    changes = 1;
    expect("DSCSSCS", 1, 0, 1333333333, 4333333333);
    fresh();
    changes = 2;
    expect("DSCSSCSPM", 2, 0, 1000000019, 17);
    const uint64_t bad_seq[] = {0, 1, UINT64_MAX};
    for (unsigned i = 0; i < 3; ++i) {
        fresh();
        page[0] = bad_seq[i];
        expect("DSSPM", 2, 0, 1000000019, 17);
    }
    fresh();
    page[0] = UINT64_MAX - 1;
    changes = 1; /* next observed sequence is disabled zero */
    expect("DSCSSPM", 2, 0, 1000000019, 17);
    const uint64_t invalid[][2] = {{1,2}, {2,4}, {3,16}, {4,0},
                                  {4,1ULL<<32}, {5,6}, {7,0}, {7,6}};
    for (unsigned i = 0; i < sizeof invalid / sizeof invalid[0]; ++i) {
        fresh();
        page[invalid[i][0]] = invalid[i][1];
        expect("DSCSSCSPM", 2, 0, 1000000019, 17);
    }
}

static void boundaries(void)
{
    fresh();
    tick = UINT64_MAX;
    page[4] = 1000000000;
    page[5] = 0;
    page[6] = 1;
    page[7] = 1;
    expect("DSCS", 1, 0, UINT64_MAX, UINT64_MAX);
    page[6] = 2; /* settled + delta exceeds UINT64_MAX by one tick */
    expect("SCS", 0, EOVERFLOW, 0, 0);
    fresh();
    page[4] = 500000000;
    tick = UINT64_MAX / 2;
    page[5] = 0;
    page[6] = 0;
    page[7] = tick;
    expect("DSCS", 1, 0, UINT64_MAX - 1, 0);
    ++tick;
    expect("SCS", 0, EOVERFLOW, 0, 0);
    page[5] = tick;
    page[7] = tick;
    page[6] = tick;
    expect("SCS", 0, EOVERFLOW, 0, 0);
}

static void fallback(void)
{
    const long unsupported[] = {-ENOSYS, -ENOTSUP, 0, (long)page + 4096};
    for (unsigned i = 0; i < 4; ++i) {
        fresh();
        discovery = unsupported[i];
        expect("DPM", 2, 0, 1000000019, 17);
        expect("PM", 2, 0, 1000000019, 17);
    }
    statuses[0] = -ENOTSUP;
    expect("P", 0, ENOTSUP, 0, 0);
    statuses[0] = 0;
    statuses[1] = -EINVAL;
    expect("PM", 0, EINVAL, 0, 0);
    statuses[1] = 0;
    const struct timespec invalid[] = {{-1,0},{0,-1},{0,1000000000}};
    for (unsigned i = 0; i < 3; ++i) {
        times[0] = invalid[i];
        expect("P", 0, EOVERFLOW, 0, 0);
    }
    times[0] = times[1] = (struct timespec){18446744073LL,709551615};
    expect("PM", 2, 0, UINT64_MAX, UINT64_MAX);
    ++times[1].tv_nsec;
    expect("PM", 0, EOVERFLOW, 0, 0);
}

static void *unsupported_reader(void *unused)
{
    (void)unused;
    int rc = pthread_barrier_wait(&start);
    assert(!rc || rc == PTHREAD_BARRIER_SERIAL_THREAD);
    struct jnox_clock_pair got = jnox_clock_sample();
    assert((!strcmp(calls, "DPM") || !strcmp(calls, "PM")) && got.backend == 2 &&
           !got.error && got.monotonic_ns == 1000000019 && got.process_cpu_ns == 17);
    expect("PM", 2, 0, 1000000019, 17);
    return NULL;
}

int main(void)
{
    assert(mmap(page, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS |
                MAP_FIXED_NOREPLACE, -1, 0) == page);
    fast_and_retry();
    boundaries();
    fallback();
    fresh();
    discovery = -ENOSYS;
    pthread_t a, b;
    assert(!pthread_barrier_init(&start, NULL, 2));
    assert(!pthread_create(&a, NULL, unsupported_reader, NULL));
    assert(!pthread_create(&b, NULL, unsupported_reader, NULL));
    assert(!pthread_join(a, NULL) && !pthread_join(b, NULL));
    assert(!pthread_barrier_destroy(&start));
    puts("clock-pair SDK controls passed");
    return 0;
}
