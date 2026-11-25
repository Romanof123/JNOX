#ifndef CAPABILITY_TELEMETRY_PROTOCOL_H
#define CAPABILITY_TELEMETRY_PROTOCOL_H
#include <stdint.h>

/* Ordinary control requests exchanged with the origin over synchronous IPC. */
enum command {
    TAKE_FIRST = 1, TAKE_SECOND = 2, FREE_FIRST = 3, TAKE_MOVED = 4, FINISH = 5,
    PENDING_EXIT = 6, ACTIVE_EXIT = 7, HOLD_ACTIVE = 8, TARGET_GONE = 9,
    DRAIN_FIRST = 21, DRAIN_SECOND = 22, DRAIN_BEFORE = 23, DRAIN_ACTIVE = 24,
    CHECK_EMPTY = 25, DRAIN_FILL = 26, DRAIN_RETRY = 27, EXIT_NOW = 255
};
#define BOTH (JNOX_CAP_SEND | JNOX_CAP_DELEGATE)
#define TAG(n) (0xe910000000000000ULL | (n))
#define SAMPLE(n) (0xcafe000000000000ULL | ((uint64_t)(n) << 32) | (0x301U + 11U * (n)))
#define ACK(command) (0xace0000000000000ULL | (command))
#endif
