#include <stdint.h>
#include <time.h>

__attribute__((section(".data.private_report")))
volatile uint32_t private_result = 0;

int main(void)
{
    /* Run the calendar report at its scheduled time, five milliseconds from now. */
    struct timespec deadline;
    if (clock_gettime(CLOCK_MONOTONIC, &deadline))
        return 103;
    deadline.tv_nsec += 5000000;
    if (deadline.tv_nsec >= 1000000000) {
        ++deadline.tv_sec;
        deadline.tv_nsec -= 1000000000;
    }
    if (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, 0))
        return 104;

    for (unsigned year = 1970; year < 2100; ++year)
        if (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0))
            ++private_result;
    uint32_t days = private_result;
    return days == 32 ? (int)days : 105;
}
