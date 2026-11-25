#include <jnox/clock_pair.h>

enum { bracket_capacity = 1024 };

struct bracket {
    uint64_t generation, completed, cpu_before, mono_before, pair_cpu, pair_mono;
    uint64_t mono_after, cpu_after, backend;
};

struct clock_evidence {
    uint64_t counts[3]; // Backend 0/error, 1/page, 2/syscall.
    uint32_t bracket_count, bracket_overflow;
#ifdef AMC_BRACKET_CONTROL
    struct bracket rows[bracket_capacity];
#endif
};
_Static_assert(__builtin_offsetof(struct clock_evidence, bracket_count) == 24, "counts layout");
_Static_assert(sizeof(struct bracket) == 72, "bracket layout");
volatile struct clock_evidence amc_clock_evidence;
#ifdef AMC_BRACKET_CONTROL
_Static_assert(__builtin_offsetof(struct clock_evidence, rows) == 32, "row offset");
#endif

#ifdef AMC_RUST_PAIR
extern struct jnox_clock_pair rust_clock_sample(void);
#define sample_pair rust_clock_sample
#else
#define sample_pair jnox_clock_sample
#endif

__attribute__((noinline))
static int sample(uint64_t count, struct stamp *s)
{
#ifdef AMC_BRACKET_CONTROL
    struct bracket b = {.generation = amc_product.generation, .completed = count};
    if (!clock_ns(CLOCK_PROCESS_CPUTIME_ID, &b.cpu_before) ||
        !clock_ns(CLOCK_MONOTONIC, &b.mono_before))
        return 0;
#endif
    struct jnox_clock_pair pair = sample_pair();
#ifdef AMC_BRACKET_CONTROL
    if (!clock_ns(CLOCK_MONOTONIC, &b.mono_after) ||
        !clock_ns(CLOCK_PROCESS_CPUTIME_ID, &b.cpu_after))
        return 0;
#endif
    if (pair.error || (pair.backend != 1 && pair.backend != 2)) {
        ++amc_clock_evidence.counts[0];
        return 0;
    }
    ++amc_clock_evidence.counts[pair.backend];
#ifdef AMC_BRACKET_CONTROL
    b.pair_cpu = pair.process_cpu_ns;
    b.pair_mono = pair.monotonic_ns;
    b.backend = pair.backend;
    unsigned n = amc_clock_evidence.bracket_count;
    if (n < bracket_capacity) {
        amc_clock_evidence.rows[n] = b;
        __atomic_store_n(&amc_clock_evidence.bracket_count, n + 1, __ATOMIC_RELEASE);
    } else {
        amc_clock_evidence.bracket_overflow = 1;
    }
    if (b.pair_cpu < b.cpu_before || b.pair_cpu > b.cpu_after ||
        b.pair_mono < b.mono_before || b.pair_mono > b.mono_after)
        return 0;
#endif
    s->count = count;
    s->cpu_ns = pair.process_cpu_ns;
    s->monotonic_ns = pair.monotonic_ns;
    return 1;
}
