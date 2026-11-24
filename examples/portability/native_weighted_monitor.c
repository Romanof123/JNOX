/* Native engineering launch: native_weighted_monitor A|B share|wake.
 * The kernel maps only the documented completion word read-only at this VA.
 * The two processes have separate address spaces and link the same ELF. */
#include <stdint.h>
#include <time.h>

#define COMPLETE ((const volatile uint64_t *)0x01200000UL)
#define MAX_CHUNKS (1UL << 28)
#define WAKE_COUNT 3

struct wake_record {
    struct timespec before;
    struct timespec target;
    struct timespec resumed;
};

struct monitor_result {
    uint64_t version;
    uint64_t role;      /* A=1, B=2 */
    uint64_t arm;       /* share=1, wake=2 */
    uint64_t sequence;  /* odd during a chunk commit, even when stable */
    uint64_t chunks;
    uint64_t samples;
    uint64_t sum;
    uint64_t weighted_sum;
    uint64_t woke;
    struct wake_record wakes[WAKE_COUNT];
    uint64_t exit_status; /* written last */
};

_Static_assert(sizeof(struct wake_record) == 48 &&
               __builtin_offsetof(struct monitor_result, wakes) == 72 &&
               __builtin_offsetof(struct monitor_result, exit_status) == 216 &&
               sizeof(struct monitor_result) == 224, "native monitor evidence layout");

volatile struct monitor_result native_weighted_result;

static const unsigned char ring[16] = {3,1,4,1,5,9,2,6,5,3,5,8,9,7,9,3};

static int equal(const char *a, const char *b)
{
    if (!a)
        return 0;
    while (*a && *a == *b) {
        ++a;
        ++b;
    }
    return *a == *b;
}

__attribute__((noinline))
static void chunk(unsigned offset)
{
    const volatile unsigned char *readings = ring;
    uint64_t sum = 0;
    uint64_t weighted = 0;
    for (unsigned i = 0; i < 256; ++i) {
        unsigned phase = i & 15;
        unsigned value = readings[(phase + offset) & 15];
        sum += value;
        weighted += (phase + 1u) * value;
    }
    /* Each address space has one writer, running on the observed single CPU. */
    ++native_weighted_result.sequence;
    ++native_weighted_result.chunks;
    native_weighted_result.samples += 256;
    native_weighted_result.sum += sum;
    native_weighted_result.weighted_sum += weighted;
    ++native_weighted_result.sequence;
}

static int normalized(struct timespec t)
{
    return t.tv_sec >= 0 && t.tv_nsec >= 0 && t.tv_nsec < 1000000000L;
}

static int sleep_worker(void)
{
    for (unsigned round = 0; round < WAKE_COUNT; ++round) {
        struct timespec before, target, resumed;
        chunk(5);
        if (clock_gettime(CLOCK_MONOTONIC, &before) != 0 || !normalized(before))
            return 92;
        target = before;
        target.tv_nsec += 40000000L;
        if (target.tv_nsec >= 1000000000L) {
            if (target.tv_sec == __LONG_MAX__)
                return 93;
            ++target.tv_sec;
            target.tv_nsec -= 1000000000L;
        }
        native_weighted_result.wakes[round].before = before;
        native_weighted_result.wakes[round].target = target;
        if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0) != 0)
            return 94;
        if (clock_gettime(CLOCK_MONOTONIC, &resumed) != 0 || !normalized(resumed))
            return 95;
        if (resumed.tv_sec < target.tv_sec ||
            (resumed.tv_sec == target.tv_sec && resumed.tv_nsec < target.tv_nsec))
            return 96;
        native_weighted_result.wakes[round].resumed = resumed;
        chunk(5);
        native_weighted_result.woke = round + 1;
    }
    return *COMPLETE == 1 ? 42 : 97;
}

static int run(int argc, char **argv)
{
    if (argc != 3 || !argv || argv[3])
        return 90;
    unsigned role = equal(argv[1], "A") ? 1 : equal(argv[1], "B") ? 2 : 0;
    unsigned arm = equal(argv[2], "share") ? 1 : equal(argv[2], "wake") ? 2 : 0;
    if (!role || !arm)
        return 90;
    native_weighted_result.version = 1;
    native_weighted_result.role = role;
    native_weighted_result.arm = arm;
    if (arm == 2 && role == 2)
        return sleep_worker();

    while (*COMPLETE == 0) {
        if (native_weighted_result.chunks == MAX_CHUNKS)
            return 91;
        chunk(role == 1 ? 0 : 5);
    }
    if (*COMPLETE != 1 || !native_weighted_result.chunks)
        return 97;
    return role == 1 ? 41 : 42;
}

int main(int argc, char **argv)
{
    int status = run(argc, argv);
    native_weighted_result.exit_status = (unsigned)status;
    return status;
}
