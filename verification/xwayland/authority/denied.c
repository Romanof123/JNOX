#include <jnox/boot_resource.h>

int main(void)
{
    unsigned char marker = 0x7b;
    if (jnox_xauthority_size() != -13 || jnox_xauthority_read(&marker, 1) != -13)
        return 109;
    return marker == 0x7b ? 72 : 109;
}
