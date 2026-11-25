#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include "../native-edf/clock.h"

struct aggregate { uint64_t count, sum, sum_of_squares; };
struct aggregate sensor_total;
volatile uint32_t sensor_samples[256];
pthread_mutex_t sensor_mutex;
unsigned worker_phase;
int worker_try, worker_lock, worker_errno_after;
int main_errno_after, main_try_after, nonowner_error, busy_destroy, self_try;
int controls_mode;
void *volatile joined_aggregate;
volatile uint64_t sleep_target_ns, sleep_observed_ns;
extern void *sensor_worker(void *);

void announce_attempt(void)
{
    __atomic_store_n(&worker_phase, 1, __ATOMIC_RELEASE);
}

static int equals(const char *a, const char *b)
{
    while (*a && *a == *b) {
        ++a;
        ++b;
    }
    return *a == *b;
}

int main(int argc, char **argv)
{
    int sleep_mode = argc == 2 && equals(argv[1], "sleep-owner");
    controls_mode = argc == 2 && equals(argv[1], "controls");
    if (argc != 1 && !sleep_mode && !controls_mode)
        return 101;
    for (unsigned i = 0; i < 256; ++i)
        sensor_samples[i] = i + 1;
    errno = EINVAL;
    if (pthread_mutex_init(&sensor_mutex, NULL) || pthread_mutex_lock(&sensor_mutex))
        return 102;
    if (controls_mode) {
        self_try = pthread_mutex_trylock(&sensor_mutex);
        busy_destroy = pthread_mutex_destroy(&sensor_mutex);
        if (self_try != EBUSY || busy_destroy != EBUSY)
            return 103;
    }
    pthread_t worker;
    if (pthread_create(&worker, NULL, sensor_worker, &sensor_total))
        return 104;
    do {
        if (sched_yield())
            return 105;
    } while (!__atomic_load_n(&worker_phase, __ATOMIC_ACQUIRE));
    if (sched_yield())
        return 106;
    if (sleep_mode) {
        uint64_t now;
        if (read_ns(&now))
            return 107;
        sleep_target_ns = now + 20000000;
        if (sleep_to(sleep_target_ns) || read_ns(&now) || now < sleep_target_ns)
            return 108;
        sleep_observed_ns = now;
    }
    for (unsigned i = 0; i < 128; ++i) {
        unsigned sample = sensor_samples[i];
        ++sensor_total.count;
        sensor_total.sum += sample;
        sensor_total.sum_of_squares += sample * sample;
    }
    if (pthread_mutex_unlock(&sensor_mutex))
        return 109;
    main_try_after = pthread_mutex_trylock(&sensor_mutex);
    if (main_try_after == 0) {
        if (pthread_mutex_unlock(&sensor_mutex))
            return 110;
    } else if (main_try_after != EBUSY) {
        return 111;
    }
    void *value;
    if (pthread_join(worker, &value))
        return 112;
    joined_aggregate = value;
    if (value != &sensor_total || worker_errno_after != ENOSYS)
        return 113;
    if (pthread_mutex_lock(&sensor_mutex))
        return 114;
    int correct = sensor_total.count == 256 && sensor_total.sum == 32896 &&
                  sensor_total.sum_of_squares == 5625216;
    if (pthread_mutex_unlock(&sensor_mutex) || pthread_mutex_destroy(&sensor_mutex))
        return 115;
    if (controls_mode && (pthread_mutex_init(&sensor_mutex, NULL) ||
        pthread_mutex_lock(&sensor_mutex) || pthread_mutex_unlock(&sensor_mutex) ||
        pthread_mutex_destroy(&sensor_mutex)))
        return 116;
    main_errno_after = errno;
    return correct && main_errno_after == EINVAL ? 0 : 117;
}
