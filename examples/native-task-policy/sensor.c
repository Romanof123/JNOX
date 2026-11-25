#include <jnox/task_policy.h>

static unsigned sequence;
volatile unsigned observed_sequence;

void sample_entry(void)
{
    do {
        if (__atomic_add_fetch(&sequence, 1u, __ATOMIC_RELEASE) == 20) {
            (void)jnox_task_exit(21);
            return;
        }
    } while (jnox_task_complete() == 0);
    (void)jnox_task_exit(90);
}

void observe_entry(void)
{
    unsigned previous_sequence = 0;
    unsigned observations = 0;
    do {
        observed_sequence = __atomic_load_n(&sequence, __ATOMIC_ACQUIRE);
        if (!observed_sequence || observed_sequence == previous_sequence) {
            (void)jnox_task_fault(9);
            return;
        }
        previous_sequence = observed_sequence;
        if (++observations == 8) {
            (void)jnox_task_exit(22);
            return;
        }
    } while (jnox_task_complete() == 0);
    (void)jnox_task_exit(91);
}

int main(void) { sample_entry(); return 92; }
