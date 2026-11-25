#include <sched.h>

int main(void)
{
    return sched_yield() == 0 ? 0 : 1;
}
