#include "worker.h"

void batch_entry(void)
{
    calculate(40000000);
    (void)jnox_task_exit(24);
}

int main(void) { batch_entry(); return 94; }
