#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <unistd.h>
#include "../native-edf/clock.h"

enum { samples = 256 };
struct transform {
    uint32_t generation;
    uint32_t output[samples];
    uint32_t sum;
    int *errno_cell;
    int initial_errno, final_errno;
    _Atomic unsigned quarters, finished;
};

uint32_t sensor_input[samples];
struct transform sensor_jobs[2] = {{.generation = 1}, {.generation = 2}};
pthread_t sensor_handles[2];
void *sensor_joined[2];
int *sensor_main_errno_cell;
uint32_t sensor_main_checksum;
uint64_t sensor_deadline_ns, sensor_wake_ns;
int sensor_stale_error;

static void *transform_samples(void *arg)
{
    struct transform *job = arg;
    job->errno_cell = &errno;
    job->initial_errno = errno;
    if (job->initial_errno || close(42) != -1 || errno != ENOSYS)
        goto failed;

    for (unsigned i = 0; i < samples; ++i) {
        job->output[i] = sensor_input[i] + job->generation;
        job->sum += job->output[i];
        if ((i + 1) % 64 == 0) {
            ++job->quarters;
            if (sched_yield() || errno != ENOSYS)
                goto failed;
            if (job->generation == 1 && i == 63) {
                uint64_t now;
                if (read_ns(&now)) goto failed;
                sensor_deadline_ns = now + 20000000;
                if (sleep_to(sensor_deadline_ns) || read_ns(&sensor_wake_ns) ||
                    sensor_wake_ns < sensor_deadline_ns || errno != ENOSYS)
                    goto failed;
            }
        }
    }
    job->final_errno = errno;
    job->finished = 1;
    if (job->generation == 2)
        pthread_exit(job);
    return job;

failed:
    job->finished = 1;
    return 0;
}

int main(void)
{
    struct timespec ignored;
    sensor_main_errno_cell = &errno;
    if (clock_gettime(-1, &ignored) != -1 || errno != EINVAL)
        return 101;
    for (unsigned i = 0; i < samples; ++i)
        sensor_input[i] = i;
    for (unsigned j = 0; j < 2; ++j) {
        struct transform *job = &sensor_jobs[j];
        if (pthread_create(&sensor_handles[j], 0, transform_samples, job))
            return 102;
        if (j == 0) {
            for (unsigned i = 0; i < samples; ++i)
                sensor_main_checksum += sensor_input[i];
            if (sched_yield() || job->quarters != 1 || job->finished)
                return 103;
        } else {
            while (!job->finished)
                if (sched_yield() || errno != EINVAL) return 104;
        }
        if (errno != EINVAL || pthread_join(sensor_handles[j], &sensor_joined[j]) ||
            errno != EINVAL || sensor_joined[j] != job)
            return 105;
        if (job->quarters != 4 || job->sum != 32640 + samples * job->generation ||
            job->errno_cell == sensor_main_errno_cell || job->initial_errno ||
            job->final_errno != ENOSYS)
            return 106;
        for (unsigned i = 0; i < samples; ++i)
            if (job->output[i] != i + job->generation) return 107;
    }
    void *unchanged = &sensor_jobs[1];
    sensor_stale_error = pthread_join(sensor_handles[0], &unchanged);
    if (sensor_handles[0] == sensor_handles[1] || sensor_stale_error != ESRCH ||
        unchanged != &sensor_jobs[1] || errno != EINVAL || sensor_main_checksum != 32640)
        return 108;
    return 17;
}
