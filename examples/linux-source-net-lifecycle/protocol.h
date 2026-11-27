#ifndef JNOX_LIFECYCLE_FIXTURE_H
#define JNOX_LIFECYCLE_FIXTURE_H
#include "../linux-source-net-rx/protocol.h"
#include <unistd.h>
struct lifecycle_session { uint64_t generation, token, mac; };
#define MARK(text) ((void)write(STDOUT_FILENO, text "\n", sizeof(text "\n")-1))
static inline int reply(unsigned char value)
{
    return pipe_send(REPLY_FD,&value,1);
}
static inline int expect_reply(unsigned char expected)
{
    unsigned char received;
    return pipe_receive(REPLY_FD,&received,1) || received!=expected;
}
#endif
