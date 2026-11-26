#ifndef JNOX_PERMIT_CODEC_H
#define JNOX_PERMIT_CODEC_H

#include <stddef.h>
#include <stdint.h>

#define JNOX_PERMIT_FRAME_BYTES 44u

typedef struct {
    uint64_t generation;
    uint64_t sequence;
    uint64_t issue_ticks;
    uint64_t permit;
} jnox_permit_fields;

enum jnox_permit_status {
    JNOX_PERMIT_ARGUMENT = -1,
    JNOX_PERMIT_OK = 0,
    JNOX_PERMIT_LENGTH = 1,
    JNOX_PERMIT_CRC = 2,
    JNOX_PERMIT_FORMAT = 3,
    JNOX_PERMIT_GENERATION = 7,
    JNOX_PERMIT_SEQUENCE = 8
};

/* Both calls check for null pointers, then require exactly 44 bytes.
 * encode next requires permit = 0 or 1 (ARGUMENT on failure). The shared
 * Rust codec supplies all subsequent validation and error ordering.
 * Both calls write their entire destination only on success.
 *
 * Supply writable storage for encode. For decode, supply an initialized,
 * private frame and a separate, aligned, writable fields object. The
 * destination may be uninitialized. issue_ticks uses the guest raw counter.
 */
int32_t jnox_permit_encode(jnox_permit_fields fields, unsigned char *frame, size_t length);
int32_t jnox_permit_decode(const unsigned char *frame, size_t length, jnox_permit_fields *out);

#endif
