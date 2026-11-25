#include <stdint.h>
#include <time.h>

enum { batch_size = 256, batches_per_clock = 256, stamp_limit = 32 };
static volatile unsigned char ring[] = {2, 5, 1, 6, 3, 7, 4, 8};

struct progress {
    uint64_t count, sum;
};
struct stamp {
    uint64_t monotonic_ns, cpu_ns, count;
};

struct product {
    uint32_t version, slot, generation, active;
    struct progress buffers[2];
    uint32_t finite_values[16];
    uint32_t checkpoint_count, finite_count;
    uint64_t checkpoint_sum, finite_sum;
    uint32_t stamp_count, stamp_overflow;
    struct stamp stamps[stamp_limit];
    struct stamp final_stamp;
};
_Static_assert(sizeof(struct product) == 936, "amc_product layout");
volatile struct product amc_product;

static int same(const char *a, const char *b)
{
    while (*a && *a == *b) {
        ++a;
        ++b;
    }
    return *a == *b;
}

static int decimal(const char *s, uint32_t *out)
{
    if (!*s)
        return 0;
    uint32_t n = 0;
    for (; *s; ++s) {
        if (*s < '0' || *s > '9')
            return 0;
        unsigned digit = (unsigned)(*s - '0');
        if (n > (0xffffffffU - digit) / 10)
            return 0;
        n = 10 * n + digit;
    }
    *out = n;
    return 1;
}

static uint64_t expected(unsigned start, uint64_t count)
{
    static const unsigned char values[] = {2, 5, 1, 6, 3, 7, 4, 8};
    uint64_t total = 36 * (count / 8);
    for (unsigned i = 0; i < count % 8; ++i)
        total += values[(start + i) % 8];
    return total;
}

static void publish(struct progress progress)
{
    unsigned index = amc_product.active ^ 1U;
    amc_product.buffers[index] = progress;
    __atomic_store_n(&amc_product.active, index, __ATOMIC_RELEASE);
}

static int clock_ns(clockid_t id, uint64_t *ns)
{
    struct timespec t;
    if (clock_gettime(id, &t) || t.tv_sec < 0 || t.tv_nsec < 0 ||
        t.tv_nsec >= 1000000000 || (uint64_t)t.tv_sec > (UINT64_MAX - (uint64_t)t.tv_nsec) / 1000000000)
        return 0;
    *ns = (uint64_t)t.tv_sec * 1000000000 + (uint64_t)t.tv_nsec;
    return 1;
}

static int sample(uint64_t count, struct stamp *s)
{
    s->count = count;
    return clock_ns(CLOCK_PROCESS_CPUTIME_ID, &s->cpu_ns) &&
           clock_ns(CLOCK_MONOTONIC, &s->monotonic_ns);
}

static void remember(struct stamp s)
{
    unsigned n = amc_product.stamp_count;
    if (n && s.cpu_ns - amc_product.stamps[n-1].cpu_ns < 8000000)
        return;
    if (n == stamp_limit) {
        amc_product.stamp_overflow = 1;
        return;
    }
    amc_product.stamps[n] = s;
    __atomic_store_n(&amc_product.stamp_count, n + 1, __ATOMIC_RELEASE);
}

__attribute__((noinline))
static uint64_t reduce_batch(unsigned start)
{
    uint64_t sum = 0;
    for (unsigned i = 0; i < batch_size; ++i)
        sum += ring[(start + i) % 8];
    return sum;
}

static uint64_t finite(unsigned start, unsigned n)
{
    uint64_t sum = 0;
    for (unsigned i = 0; i < n; ++i) {
        unsigned value = ring[(start + i) % 8];
        amc_product.finite_values[i] = value;
        sum += value;
    }
    return sum;
}

int main(int argc, char **argv)
{
    uint32_t generation, amount;
    if (argc != 4 || !decimal(argv[1], &generation) || generation >= 16 ||
        !decimal(argv[3], &amount) || !amount)
        return 101;
    int short_job = same(argv[2], "finite");
    int survivor = same(argv[2], "survivor");
    if ((!short_job && !survivor && !same(argv[2], "stream")) ||
        (short_job && amount != 8 && amount != 16))
        return 102;

    amc_product.version = 1;
    amc_product.slot = WORKER_ID;
    amc_product.generation = generation;
    unsigned start = (WORKER_ID + generation) % 8;
    struct progress progress = {0, 0};
    struct stamp stamp;
    if (!sample(0, &stamp))
        return 103;
    remember(stamp);
    if (survivor) {
        amc_product.checkpoint_sum = finite(start, 8);
        amc_product.checkpoint_count = 8;
    }
    if (!short_job) {
        uint64_t end = (uint64_t)amount * 1000000;
        do {
            for (unsigned i = 0; i < batches_per_clock; ++i) {
                if (progress.count > UINT64_MAX - batch_size)
                    return 104;
                progress.sum += reduce_batch(start);
                progress.count += batch_size;
                publish(progress);
            }
            if (!sample(progress.count, &stamp))
                return 105;
            remember(stamp);
        } while (stamp.cpu_ns < end);
    }
    if (short_job || survivor) {
        unsigned n = short_job ? amount : 16;
        amc_product.finite_sum = finite(start, n);
        amc_product.finite_count = n;
        if (amc_product.finite_sum != expected(start, n))
            return 106;
        if (short_job) {
            progress = (struct progress){n, amc_product.finite_sum};
            publish(progress);
        }
    }
    if (progress.sum != expected(start, progress.count) ||
        (survivor && amc_product.checkpoint_sum != 36))
        return 107;
    if (!sample(progress.count, &stamp))
        return 108;
    amc_product.final_stamp = stamp;
    return WORKER_ID;
}
