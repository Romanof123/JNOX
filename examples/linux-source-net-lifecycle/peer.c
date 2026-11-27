#include "protocol.h"

volatile uint64_t jnox_lifecycle_peer[2][12];
unsigned char jnox_lifecycle_peer_frames[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_lifecycle_peer_ack[ACK_SIZE];
unsigned char jnox_lifecycle_peer_second[2][ACK_SIZE];

static int session(unsigned g)
{
    struct lifecycle_session s;
    volatile uint64_t *p=jnox_lifecycle_peer[g];
    if (pipe_receive(DATA_FD,&s,sizeof(s)) || s.generation!=g || !s.token ||
        (g && s.token==jnox_lifecycle_peer[0][2])) return 40;
    p[0]=0x4c49464550454552ULL;
    p[2]=s.token; p[3]=s.mac;
    p[7]=native_call(172,s.token,0,0);
    p[8]=native_call(174,s.token,0,0);
    p[9]=native_call(175,s.token,0,0);
    if (p[7]!=(uint64_t)-13L || p[8]!=(uint64_t)-13L || p[9]!=(uint64_t)-13L ||
        reply(0xa3)) return 41;
    for (unsigned seq=0; seq<(g ? 1u : FRAME_COUNT); ++seq) {
        unsigned char *buf=g ? jnox_lifecycle_peer_second[0] : jnox_lifecycle_peer_frames[seq];
        unsigned len=frame_length(seq);
        if (pipe_receive(DATA_FD,buf,len)) return 42;
        if (!seq) p[4]=read_be(buf+22,8);
        if ((g && p[4]==jnox_lifecycle_peer[0][4]) ||
            !valid_frame(buf,len,seq,s.mac,p[4])) return 43;
        ++p[5]; p[6]+=len;
    }
    unsigned char *ack=g ? jnox_lifecycle_peer_second[1] : jnox_lifecycle_peer_ack;
    if (reply(0xa6) || pipe_receive(DATA_FD,ack,ACK_SIZE) ||
        !valid_ack(ack,ACK_SIZE,s.mac,p[4]) || reply(0xa7)) return 44;
    p[10]=ACK_SIZE; p[1]=100;
    return 0;
}

int main(void)
{
    for (unsigned g=0; g<2; ++g) {
        int rc=session(g);
        jnox_lifecycle_peer[g][11]=rc;
        if (rc) return rc;
    }
    MARK("NET LIFECYCLE PEER PASS");
    return 0;
}
