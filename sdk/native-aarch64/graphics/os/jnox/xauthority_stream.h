#ifndef JNOX_XAUTHORITY_STREAM_H
#define JNOX_XAUTHORITY_STREAM_H
#include <stddef.h>
#include <stdio.h>
#include <jnox/boot_resource.h>

FILE *jnox_xauthority_open(const char *path, const char *mode,
                           unsigned char storage[JNOX_XAUTHORITY_MAX],
                           size_t *length);
void jnox_xauthority_wipe(void *buf, size_t size);
#endif
