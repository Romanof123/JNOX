#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <unistd.h>
#include "../native-edf/clock.h"

enum { block_bytes = 4096, full_blocks = 16384 };
static volatile unsigned char main_ramp[block_bytes], worker_ramp[block_bytes];
volatile uint64_t main_sum, worker_bins[16];
volatile uint32_t main_blocks, worker_blocks;
volatile uint64_t sleep_target_ns, sleep_observed_ns;
volatile int main_errno_after, worker_errno_after;
void *volatile joined_result;

struct worker_descriptor {
    unsigned blocks;
    uint64_t sleep_ns;
    int check_errno;
};
struct worker_descriptor worker_job = {full_blocks, 0, 0};

__attribute__((noinline))
void main_reduce(unsigned blocks)
{
    for (unsigned block = 0; block < blocks; ++block) {
        unsigned sum = 0;
        for (unsigned i = 0; i < block_bytes; ++i)
            sum += main_ramp[i];
        main_sum += sum;
        ++main_blocks;
    }
}

__attribute__((noinline))
void worker_reduce(unsigned blocks)
{
    for (unsigned block = 0; block < blocks; ++block) {
        for (unsigned i = 0; i < block_bytes; ++i)
            ++worker_bins[worker_ramp[i] >> 4];
        ++worker_blocks;
    }
}

static void *worker(void *arg)
{
    const struct worker_descriptor *job = arg;
    if (job->sleep_ns) {
        uint64_t now;
        if (read_ns(&now))
            return NULL;
        sleep_target_ns = now + job->sleep_ns;
        if (sleep_to(sleep_target_ns) || read_ns(&now) || now < sleep_target_ns)
            return NULL;
        sleep_observed_ns = now;
    }
    if (job->check_errno && (close(-1) != -1 || errno != ENOSYS))
        return NULL;
    worker_reduce(job->blocks);
    if (job->check_errno) {
        worker_errno_after = errno;
        if (worker_errno_after != ENOSYS)
            return NULL;
    }
    return arg;
}

static int matches(const char *a, const char *b)
{
    while (*a && *a == *b) {
        ++a;
        ++b;
    }
    return *a == *b;
}

int main(int argc, char **argv)
{
    int late_create = 0, yield_first = 0;
    unsigned after_create = full_blocks;
    if (argc == 2) {
        const char *mode = argv[1];
        if (matches(mode, "early-exit"))
            worker_job.blocks = 1;
        else if (matches(mode, "early-wake") || matches(mode, "late-wake")) {
            worker_job.sleep_ns = matches(mode, "early-wake") ? 3000000 : 25000000;
            yield_first = 1;
        } else if (matches(mode, "idle-join")) {
            worker_job.sleep_ns = 20000000;
            worker_job.blocks = 1;
            after_create = 1;
        } else if (matches(mode, "late-create")) {
            late_create = 1;
        } else if (matches(mode, "errno")) {
            worker_job.check_errno = 1;
        } else {
            return 101;
        }
    } else if (argc != 1) {
        return 101;
    }

    for (unsigned i = 0; i < block_bytes; ++i) {
        main_ramp[i] = (unsigned char)i;
        worker_ramp[i] = (unsigned char)(17 * i + 3);
    }
    if (late_create)
        main_reduce(full_blocks);
    if (worker_job.check_errno) {
        struct timespec ignored;
        if (clock_gettime(-1, &ignored) != -1 || errno != EINVAL)
            return 102;
    }
    pthread_t handle;
    if (pthread_create(&handle, NULL, worker, &worker_job))
        return 103;
    if (yield_first && sched_yield())
        return 104;
    main_reduce(after_create);
    if (worker_job.check_errno) {
        main_errno_after = errno;
        if (main_errno_after != EINVAL)
            return 105;
    }
    void *result;
    if (pthread_join(handle, &result))
        return 106;
    joined_result = result;
    if (result != &worker_job || worker_blocks != worker_job.blocks)
        return 107;
    for (unsigned i = 0; i < 16; ++i)
        if (worker_bins[i] != (uint64_t)worker_job.blocks * 256)
            return 108;
    unsigned expected = after_create + (late_create ? full_blocks : 0);
    return main_blocks == expected && main_sum == 522240ULL * expected ? 17 : 109;
}
