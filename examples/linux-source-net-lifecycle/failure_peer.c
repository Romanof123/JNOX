#include "protocol.h"

volatile uint64_t jnox_failure_peer[5][8];
unsigned char jnox_failure_recovery_bytes[2][ACK_SIZE];

static int observe(unsigned id)
{
    struct lifecycle_session s;
    volatile uint64_t *p=jnox_failure_peer[id-1];
    if (pipe_receive(DATA_FD,&s,sizeof(s)) || s.generation!=id ||
        (!!s.token != (id!=1))) return 50;
    p[0]=id;
    p[1]=s.token;
    p[2]=native_call(172,s.token,0,0);
    p[3]=native_call(174,s.token,0,0);
    p[4]=native_call(175,s.token,0,0);
    if (p[2]!=(uint64_t)-13L || p[3]!=(uint64_t)-13L ||
        p[4]!=(uint64_t)-13L || reply(0xa3)) return 51;
    if (id==5) {
        for (unsigned i=0; i<2; ++i) {
            unsigned char *f=jnox_failure_recovery_bytes[i];
            if (pipe_receive(DATA_FD,f,ACK_SIZE)) return 52;
            if (!i) p[5]=read_be(f+22,8);
            if (!(i ? valid_ack(f,ACK_SIZE,s.mac,p[5]) : valid_frame(f,ACK_SIZE,0,s.mac,p[5])) ||
                reply(i ? 0xa7 : 0xa6)) return 53;
            ++p[6];
        }
    }
    return 0;
}

int main(void)
{
    for (unsigned id=1; id<=5; ++id) {
        int rc=observe(id);
        jnox_failure_peer[id-1][7]=rc;
        if (rc) return rc;
    }
    MARK("NET FAILURE PEER PASS");
    return 0;
}
