#include <stddef.h>

void *memset(void *destination, int byte, size_t count)
{
    unsigned char *out = destination;
    for (size_t i = 0; i < count; ++i)
        out[i] = (unsigned char)byte;
    return destination;
}
