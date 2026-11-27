/* Freestanding reader for the FamilyLocal record used by Mosaic on display 0. */
#ifndef MOSAIC_XAUTHORITY_H
#define MOSAIC_XAUTHORITY_H

#include <stddef.h>

#define MOSAIC_AUTH_PROTOCOL "MIT-MAGIC-COOKIE-1"
enum { MOSAIC_COOKIE_SIZE = 16 };

struct auth_field {
    const unsigned char *bytes;
    size_t size;
};

static int read_auth_field(struct auth_field *input, struct auth_field *field)
{
    if (input->size < 2) return 0;
    size_t size = ((unsigned)input->bytes[0] << 8) | input->bytes[1];
    if (size > input->size - 2) return 0;
    *field = (struct auth_field){ input->bytes + 2, size };
    input->bytes += 2 + size;
    input->size -= 2 + size;
    return 1;
}

static int auth_matches(struct auth_field field, const char *text, size_t size)
{
    if (field.size != size) return 0;
    for (size_t i = 0; i < size; ++i)
        if (field.bytes[i] != (unsigned char)text[i]) return 0;
    return 1;
}

static int local_auth_cookie(const unsigned char *bytes, size_t size,
                             const char *node, size_t node_size,
                             unsigned char cookie[MOSAIC_COOKIE_SIZE])
{
    struct auth_field input = { bytes, size };
    const unsigned char *match = NULL;
    if (!node_size) return 0;
    while (input.size) {
        if (input.size < 2) return 0;
        unsigned family = ((unsigned)input.bytes[0] << 8) | input.bytes[1];
        input.bytes += 2;
        input.size -= 2;
        struct auth_field host, number, protocol, data;
        if (!read_auth_field(&input, &host) || !read_auth_field(&input, &number) ||
            !read_auth_field(&input, &protocol) || !read_auth_field(&input, &data)) return 0;
        if (!match && family == 256 && auth_matches(host, node, node_size) &&
            auth_matches(number, "0", 1) &&
            auth_matches(protocol, MOSAIC_AUTH_PROTOCOL, sizeof(MOSAIC_AUTH_PROTOCOL) - 1) &&
            data.size == MOSAIC_COOKIE_SIZE)
            match = data.bytes;
    }
    if (!match) return 0;
    for (size_t i = 0; i < MOSAIC_COOKIE_SIZE; ++i) cookie[i] = match[i];
    return 1;
}
#endif
