#include <jnox/task_policy.h>

void report_entry(void)
{
    (void)jnox_task_fault(37);
    (void)jnox_task_exit(95);
}

int main(void) { report_entry(); return 96; }
