#include "protocol.h"
#include <time.h>

extern int jnox_packet_context_enter_v1(unsigned int);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern int jnox_lifecycle_probe_v1(void), jnox_lifecycle_open_v1(void);
extern int jnox_lifecycle_close_v1(void), jnox_lifecycle_remove_v1(void);
extern void jnox_lifecycle_stats_v1(unsigned long long out[12]);
extern unsigned long jnox_packet_token(void);
extern unsigned long jnox_rx_posted_address_v1(unsigned int);
extern int jnox_original_rx_read(void *, unsigned int);

enum phase { PROBED, OPENED, DELIVERED, CLOSED, REOPENED, RECLOSED, REMOVED, PHASES };
volatile uint64_t jnox_lifecycle_driver[2][16];
unsigned long long jnox_lifecycle_ops[2][PHASES][12];
uint64_t jnox_lifecycle_state[2][PHASES][5];
static unsigned char frame[1514];

/* A quiescent observation interval for read-only QMP. Both lifecycle
 * transitions themselves and the NIC observations precede this sleep. */
static int hold(unsigned g, unsigned stage)
{
    struct timespec t;
    jnox_lifecycle_driver[g][1]=stage;
    if (clock_gettime(CLOCK_MONOTONIC,&t)) return 28;
    ++t.tv_sec;
    return clock_nanosleep(CLOCK_MONOTONIC,TIMER_ABSTIME,&t,0) ? 28 : 0;
}

static int configure(struct lifecycle_session *s, volatile uint64_t *d)
{
    long low=native_call(157,0,0,0), high=native_call(157,1,0,0);
    if (low<0 || high<0 || !(low&32) || !(high&1)) return 12;
    for (unsigned i=0; i<6; ++i) {
        long b=native_call(158,i,1,0);
        if (b<0 || b>255) return 13;
        s->mac=(s->mac<<8)|(unsigned)b;
    }
    unsigned mtu=0, reads=0, diagnostics=0;
    unsigned long long mask=(1ULL<<32)|((unsigned)low&40u);
    if (jnox_net_validate(&mask,0,&mtu,&reads,&diagnostics) ||
        diagnostics || reads!=!!(low&8) ||
        mask!=((1ULL<<32)|32|((low&8)&&mtu>=68 ? 8 : 0))) return 14;
    d[7]=mask; d[8]=mtu; d[9]=reads;
    return native_call(159,(uint32_t)mask,mask>>32,0) ? 15 : 0;
}

static int record(unsigned g, enum phase p, uint64_t token)
{
    unsigned long long *ops=jnox_lifecycle_ops[g][p];
    uint64_t *s=jnox_lifecycle_state[g][p];
    jnox_lifecycle_stats_v1(ops);
    s[0]=native_call(160,0,0,0);
    s[1]=native_call(162,token,0,0);
    s[2]=native_call(162,token,1,0);
    for (unsigned i=0; i<32; ++i)
        if (jnox_rx_posted_address_v1(i)) ++s[3];
    s[4]=jnox_packet_token();
    unsigned opens=(g ? 2u : 0u)+(p>=OPENED)+(!g && p>=REOPENED);
    unsigned closes=(g ? 2u : 0u)+(p>=CLOSED)+(!g && p>=RECLOSED);
    if (ops[7]!=g+1 || ops[8]!=opens || ops[9]!=closes ||
        ops[10]!=g+(p==REMOVED) || ops[11]!=token) return 1;
    if (p==REMOVED)
        return s[0]!=(uint64_t)-19L || s[1]!=(uint64_t)-116L ||
               s[2]!=(uint64_t)-116L || s[3] || s[4];
    if (s[0]!=15 || (s[1]&0xffffffffULL)!=0x200001 ||
        (s[2]&0xffffffffULL)!=0x200001 || s[4]!=token) return 1;
    for (unsigned i=0; i<=6; ++i) if (!ops[i]) return 1;
    if (p==PROBED && s[3]) return 1;
    if ((p==OPENED || p==REOPENED) && s[3]!=32) return 1;
    return 0;
}

static int read_frame(void)
{
    int n;
    do { n=jnox_original_rx_read(frame,sizeof(frame)); } while (n==-11);
    return n;
}

#ifdef JNOX_LIFECYCLE_ASSERT_TEST
volatile uint64_t jnox_assert_driver[8];
volatile uint64_t jnox_assert_rx_slots[32];

__attribute__((noinline, noreturn)) void jnox_lifecycle_assert_trap(void)
{
    __asm__ volatile("brk #0x440" ::: "memory");
    __builtin_unreachable();
}

static int assert_exchange(struct lifecycle_session *s, volatile uint64_t *d)
{
    if (pipe_send(DATA_FD,s,sizeof(*s)) || expect_reply(0xa3)) return 20;
    MARK("NET ASSERT FIRST READY");
    int n=read_frame();
    if (n!=(int)frame_length(0)) return 21;
    d[4]=read_be(frame+22,8);
    if (!d[4] || !valid_frame(frame,n,0,s->mac,d[4]) ||
        pipe_send(DATA_FD,frame,n) || expect_reply(0xa6)) return 22;
    d[5]=1;
    d[6]=n;
    if (record(0,DELIVERED,s->token)) return 25;
    unsigned count=0;
    for (unsigned i=0; i<32; ++i) {
        jnox_assert_rx_slots[i]=jnox_rx_posted_address_v1(i);
        count+=jnox_assert_rx_slots[i]!=0;
    }
    if (!count || count!=jnox_lifecycle_state[0][DELIVERED][3]) return 29;
    jnox_assert_driver[0]=s->token;
    jnox_assert_driver[1]=d[4];
    jnox_assert_driver[2]=n;
    jnox_assert_driver[3]=count;
    jnox_assert_driver[4]=jnox_lifecycle_state[0][DELIVERED][0];
    jnox_assert_driver[5]=jnox_lifecycle_state[0][DELIVERED][1];
    jnox_assert_driver[6]=jnox_lifecycle_state[0][DELIVERED][2];
    jnox_assert_driver[7]=1;
    MARK("NET ASSERT ARMED");
    if (hold(0,3)) return 28;
    jnox_lifecycle_assert_trap();
}
#else
static int exchange(struct lifecycle_session *s, volatile uint64_t *d)
{
    unsigned g=s->generation;
    if (pipe_send(DATA_FD,s,sizeof(*s)) || expect_reply(0xa3)) return 20;
    if (!g) MARK("NET LIFECYCLE FIRST READY"); else MARK("NET LIFECYCLE SECOND READY");
    for (unsigned seq=0; seq<(g ? 1u : FRAME_COUNT); ++seq) {
        int n=read_frame();
        if (n!=(int)frame_length(seq)) return 21;
        if (!seq) d[4]=read_be(frame+22,8);
        if ((g && d[4]==jnox_lifecycle_driver[0][4]) ||
            !valid_frame(frame,n,seq,s->mac,d[4]) || pipe_send(DATA_FD,frame,n)) return 22;
        ++d[5]; d[6]+=n;
    }
    if (expect_reply(0xa6)) return 23;
    if (!g) MARK("NET LIFECYCLE FIRST ACK"); else MARK("NET LIFECYCLE SECOND ACK");
    int n=read_frame();
    if (n!=ACK_SIZE || !valid_ack(frame,n,s->mac,d[4]) || pipe_send(DATA_FD,frame,n) ||
        expect_reply(0xa7)) return 24;
    d[10]=n;
    return 0;
}

#endif

static int generation(unsigned g)
{
    volatile uint64_t *d=jnox_lifecycle_driver[g];
    struct lifecycle_session s={.generation=g};
    d[0]=0x4c49464544525631ULL;
    if (native_call(156,1,0,0) || jnox_packet_context_enter_v1(0)) return 10;
    int err=configure(&s,d);
    jnox_packet_context_leave_v1();
    if (err) return err;
    if (jnox_lifecycle_probe_v1()) return 16;
    s.token=jnox_packet_token();
    d[2]=s.token; d[3]=s.mac;
    if (!s.token || (g && s.token==jnox_lifecycle_driver[0][2]) ||
        record(g,PROBED,s.token)) return 17;
    if (hold(g,1)) return 28;
    if (g) {
        d[11]=native_call(162,jnox_lifecycle_driver[0][2],0,0);
        if (d[11]!=(uint64_t)-116L) return 18;
    }
    if (jnox_lifecycle_open_v1() || record(g,OPENED,s.token)) return 19;
    d[1]=2;
#ifdef JNOX_LIFECYCLE_ASSERT_TEST
    return assert_exchange(&s,d);
#else
    err=exchange(&s,d);
    if (err) return err;
    if (record(g,DELIVERED,s.token) || jnox_lifecycle_close_v1() ||
        record(g,CLOSED,s.token)) return 25;
    d[1]=3;
    if (!g && (jnox_lifecycle_open_v1() || record(g,REOPENED,s.token) ||
               jnox_lifecycle_close_v1() || record(g,RECLOSED,s.token))) return 26;
    if (jnox_lifecycle_remove_v1() || record(g,REMOVED,s.token)) return 27;
    if (!g && hold(g,4)) return 28;
    d[1]=100;
    return 0;
#endif
}

int main(void)
{
#ifdef JNOX_LIFECYCLE_ASSERT_TEST
    int rc=generation(0);
    jnox_lifecycle_driver[0][15]=rc;
    return rc ? rc : 30;
#else
    for (unsigned g=0; g<2; ++g) {
        int rc=generation(g);
        jnox_lifecycle_driver[g][15]=rc;
        if (rc) return rc;
    }
    MARK("NET LIFECYCLE DRIVER PASS");
    return 0;
#endif
}
