#include <errno.h>
#include <jnox/periodic.h>

volatile int recorded_result;
volatile int recorded_errno;

int main(void)
{
    recorded_result = jnox_periodic_wait_next();
    recorded_errno = errno;
    return recorded_result == -1 && recorded_errno == ENOTSUP ? 59 : 117;
}
