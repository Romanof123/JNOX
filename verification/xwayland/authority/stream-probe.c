/* Run only with an authorized role and privately generated valid record. */
#include <errno.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <jnox/xauthority_stream.h>

int main(void)
{
    unsigned char storage[JNOX_XAUTHORITY_MAX];
    size_t n;
    FILE *f = jnox_xauthority_open(JNOX_XAUTHORITY_PATH, "w", storage, &n);
    if (f || errno != EROFS) return 1;
    f = jnox_xauthority_open("/unknown", "r", storage, &n);
    if (f || errno != ENOENT) return 2;
    f = jnox_xauthority_open(JNOX_XAUTHORITY_PATH, "rb", storage, &n);
    if (!f || !n) return 3;
    for (size_t i = 0; i < n; ++i)
        if (fgetc(f) != storage[i]) return 4;
    if (fgetc(f) != EOF || !feof(f) || ftell(f) != (long)n) return 5;
    if (fputc(0, f) != EOF) return 6;
    if (fclose(f)) return 7;
    jnox_xauthority_wipe(storage, sizeof(storage));
    for (size_t i = 0; i < sizeof(storage); ++i)
        if (storage[i]) return 8;
    return 0;
}
