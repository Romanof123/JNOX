#include "protocol.h"

volatile uint64_t jnox_lifecycle_peer[2][12];
unsigned char jnox_lifecycle_peer_frames[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_lifecycle_peer_ack[ACK_SIZE];
unsigned char jnox_lifecycle_peer_second[2][ACK_SIZE];

#ifdef JNOX_LIFECYCLE_ASSERT_TEST
#include <time.h>
volatile uint64_t jnox_assert_peer[12];
unsigned char jnox_assert_peer_frame[MAX_FRAME];

static int observe_fault(void)
{
    struct lifecycle_session s;
    volatile uint64_t *r=jnox_assert_peer;
    if (pipe_receive(DATA_FD,&s,sizeof(s)) || s.generation || !s.token) return 40;
    r[0]=s.token;
    r[1]=s.mac;
    if (reply(0xa3) || pipe_receive(DATA_FD,jnox_assert_peer_frame,frame_length(0))) return 41;
    r[2]=read_be(jnox_assert_peer_frame+22,8);
    r[3]=frame_length(0);
    if (!r[2] || !valid_frame(jnox_assert_peer_frame,r[3],0,s.mac,r[2]) || reply(0xa6))
        return 42;
    r[4]=1;
    unsigned char extra;
    long eof=native_call(46,DATA_FD,(uintptr_t)&extra,1);
    r[5]=eof;
    if (eof) return 43;
    struct timespec start, target, end;
    if (clock_gettime(CLOCK_MONOTONIC,&start)) return 44;
    target=start;
    target.tv_nsec+=100000000;
    if (target.tv_nsec>=1000000000) { target.tv_nsec-=1000000000; ++target.tv_sec; }
    if (clock_nanosleep(CLOCK_MONOTONIC,TIMER_ABSTIME,&target,0) ||
        clock_gettime(CLOCK_MONOTONIC,&end)) return 45;
    r[6]=(uint64_t)start.tv_sec*1000000000+start.tv_nsec;
    r[7]=(uint64_t)end.tv_sec*1000000000+end.tv_nsec;
    if (r[7]<r[6] || r[7]-r[6]<100000000) return 46;
    r[8]=native_call(172,s.token,0,0);
    r[9]=native_call(174,s.token,0,0);
    r[10]=native_call(175,s.token,0,0);
    if (r[8]!=(uint64_t)-13L || r[9]!=(uint64_t)-13L || r[10]!=(uint64_t)-13L)
        return 47;
    r[11]=100;
    MARK("NET ASSERT PEER PASS");
    return 0;
}
#else
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

#endif

int main(void)
{
#ifdef JNOX_LIFECYCLE_ASSERT_TEST
    return observe_fault();
#else
    for (unsigned g=0; g<2; ++g) {
        int rc=session(g);
        jnox_lifecycle_peer[g][11]=rc;
        if (rc) return rc;
    }
    MARK("NET LIFECYCLE PEER PASS");
    return 0;
#endif
}
