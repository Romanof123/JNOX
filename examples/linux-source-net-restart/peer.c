#include "protocol.h"

volatile uint64_t jnox_restart_peer[32];
/* The host releases only these two test gates after stopped-state inspection. */
volatile uint64_t jnox_restart_gates[2];
unsigned char jnox_restart_primary_frame[60];
unsigned char jnox_restart_frames[FRAME_COUNT][MAX_FRAME];
unsigned char jnox_restart_ack[ACK_SIZE];
#define P jnox_restart_peer

static int tick(unsigned phase)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC,&t)) return -1;
    uint64_t start=(uint64_t)t.tv_sec*1000000000ULL+t.tv_nsec;
    uint64_t target=start+5000000ULL;
    struct timespec wake={(time_t)(target/1000000000ULL),(long)(target%1000000000ULL)};
    if (clock_nanosleep(CLOCK_MONOTONIC,TIMER_ABSTIME,&wake,0) ||
        clock_gettime(CLOCK_MONOTONIC,&t)) return -1;
    uint64_t end=(uint64_t)t.tv_sec*1000000000ULL+t.tv_nsec;
    if (end<target || (P[12] && start<P[12])) return -1;
    ++P[9+phase];
    P[12]=end;
    if (!P[13+phase]) P[13+phase]=start;
    P[16+phase]=end;
    return 0;
}

static int await_gate(unsigned index, uint64_t expected)
{
    do {
        if (tick(index)) return -1;
        if (jnox_restart_gates[index] && jnox_restart_gates[index]!=expected) return -1;
    } while (jnox_restart_gates[index]!=expected);
    return 0;
}

static int deny(uint64_t token, unsigned i)
{
    P[i]=native_call(174,token,0,0);
    P[i+1]=native_call(175,token,0,0);
    P[i+2]=native_call(172,token,0,0);
    return P[i]!=(uint64_t)-13L || P[i+1]!=(uint64_t)-13L || P[i+2]!=(uint64_t)-13L;
}

static int run(void)
{
    struct session first, second;
    if (pipe_receive(DATA_FD,&first,sizeof(first)) || !first.token ||
        deny(first.token,20) || signal_byte(REPLY_FD,0xa1)) return 40;
    P[3]=first.token;
    if (pipe_receive(DATA_FD,jnox_restart_primary_frame,60)) return 41;
    P[5]=read_be(jnox_restart_primary_frame+22,8);
    if (!valid_frame(jnox_restart_primary_frame,60,0,first.mac,P[5])) return 42;
    P[1]=2;
    MARK("NET RESTART OLD BARRIER");
    if (await_gate(0,RELEASE_EXIT) || signal_byte(REPLY_FD,0xe1)) return 43;
    if (await_byte(SPARE_DATA,0xb2)) return 44;
    P[1]=3;
    MARK("NET RESTART MIDDLE BARRIER");
    if (await_gate(1,RELEASE_BIND)) return 45;
    struct handoff old={first.token,P[5]};
    if (pipe_send(SPARE_REPLY,&old,sizeof(old)) ||
        pipe_receive(SPARE_DATA,&second,sizeof(second)) || !second.token ||
        second.token==first.token || second.mac!=first.mac) return 46;
    P[4]=second.token;
    if (deny(second.token,23) || signal_byte(SPARE_REPLY,0xa2)) return 47;
    P[1]=4;
    for (unsigned seq=0; seq<FRAME_COUNT; ++seq) {
        unsigned char *buf=jnox_restart_frames[seq];
        unsigned len=frame_length(seq);
        if (pipe_receive(SPARE_DATA,buf,len)) return 48;
        if (!seq) P[6]=read_be(buf+22,8);
        if (P[5]==P[6] || !valid_frame(buf,len,seq,second.mac,P[6])) return 49;
        ++P[7]; P[8]+=len;
        if (!seq && tick(2)) return 50;
    }
    if (signal_byte(SPARE_REPLY,0xa6) || pipe_receive(SPARE_DATA,jnox_restart_ack,ACK_SIZE) ||
        !valid_ack(jnox_restart_ack,ACK_SIZE,second.mac,P[6]) || signal_byte(SPARE_REPLY,0xa7))
        return 51;
    P[26]=ACK_SIZE;
    P[1]=100;
    return 0;
}
int main(void)
{
    P[0]=0x5245535450454552ULL;
    P[1]=1;
    int rc=run();
    P[2]=rc;
    return rc;
}
