#include <errno.h>
#include <jnox/loan.h>
#include <time.h>

volatile uint64_t loan_write_handles[8], loan_write_vas[8], loan_abandoned_handle;
volatile long loan_full_status;
volatile unsigned loan_full_attempts;

/* Yielding to a lower-priority consumer requires blocking, even under RR. */
static int let_reader_run(void) {
    struct timespec wake;
    if (clock_gettime(CLOCK_MONOTONIC, &wake) != 0) return 0;
    wake.tv_nsec += 10000000;
    if (wake.tv_nsec >= 1000000000) { ++wake.tv_sec; wake.tv_nsec -= 1000000000; }
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &wake, 0) == 0;
}

int main(void) {
    for (unsigned sequence = 1; sequence <= 8; ++sequence) {
        jnox_writer_loan w;
        do {
            w = jnox_loan_acquire();
            if (w.status == -EAGAIN && !let_reader_run()) return 81;
        } while (w.status == -EAGAIN);
        if (w.status != 0) return 82;
        loan_write_handles[sequence - 1] = w.handle;
        loan_write_vas[sequence - 1] = (uint64_t)w.page;
        size_t length = 5 + 3 * sequence;
        for (size_t i = 0; i < length; ++i) {
            if (w.page[i] != 0) return 83;
            w.page[i] = (unsigned char)(sequence * 17 + 3 * i);
        }
        if (w.page[4095] != 0) return 84;
        w.page[4095] = 0xa5; /* excluded from each published length */
        long status = jnox_loan_publish(w.handle, length);
        if (sequence == 3) {
            loan_full_status = status;
            if (status != -EAGAIN) return 85;
        }
        while (status == -EAGAIN) {
            ++loan_full_attempts;
            if (!let_reader_run()) return 86;
            /* The same writer retains both the payload and excluded tail. */
            if (w.page[0] != sequence * 17 || w.page[4095] != 0xa5) return 87;
            status = jnox_loan_publish(w.handle, length);
        }
        if (status != 0) return 88;
    }
    /* Two or fewer queued pages here; a CPU-preempted reader can hold the third. */
    for (;;) {
        jnox_writer_loan last = jnox_loan_acquire();
        if (last.status == 0) { loan_abandoned_handle = last.handle; return 73; }
        if (last.status != -EAGAIN || !let_reader_run()) return 89;
    }
}
