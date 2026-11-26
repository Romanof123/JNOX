#include <jnox/permit_codec.h>

/* This round trip uses a fixed guest counter tick for a repeatable example. */
int main(void)
{
    jnox_permit_fields issued = {
        .generation = 1, .sequence = 2, .issue_ticks = 625000, .permit = 1
    };
    unsigned char frame[JNOX_PERMIT_FRAME_BYTES];
    jnox_permit_fields received;

    if (jnox_permit_encode(issued, frame, sizeof frame) != JNOX_PERMIT_OK)
        return 1;
    if (jnox_permit_decode(frame, sizeof frame, &received) != JNOX_PERMIT_OK)
        return 2;
    if (received.generation != issued.generation || received.sequence != issued.sequence ||
        received.issue_ticks != issued.issue_ticks || received.permit != issued.permit)
        return 3;
    return 0;
}
