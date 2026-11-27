#ifndef JNOX_SOURCE_NET_RESTART_PROTOCOL_H
#define JNOX_SOURCE_NET_RESTART_PROTOCOL_H
#include "../linux-source-net-rx/protocol.h"
#include <time.h>
#include <unistd.h>

#define MARK(s) ((void)write(STDOUT_FILENO, s "\n", sizeof(s "\n") - 1))
enum { SPARE_DATA = 5, SPARE_REPLY = 6, RELEASE_EXIT = 0x45584954, RELEASE_BIND = 0x42494e44 };
struct session { uint64_t token, mac; };
struct handoff { uint64_t old_token, old_nonce; };
static inline int signal_byte(unsigned fd, unsigned char value)
{
    return pipe_send(fd, &value, 1);
}
static inline int await_byte(unsigned fd, unsigned char expected)
{
    unsigned char actual;
    return pipe_receive(fd, &actual, 1) || actual != expected;
}
#endif
