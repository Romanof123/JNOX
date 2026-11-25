#include "worker.h"

void burn_entry(void)
{
    calculate(100000000);
    (void)jnox_task_exit(23);
}

int main(void) { burn_entry(); return 93; }
