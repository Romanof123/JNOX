#include "protocol.h"
#include "../../drivers/linux-compat/packet/include/lifecycle_faults.h"
#include "../../drivers/linux-compat/packet/include/lifecycle_netdev.h"
#include <time.h>

extern int jnox_packet_context_enter_v1(unsigned);
extern void jnox_packet_context_leave_v1(void);
extern long jnox_net_validate(unsigned long long *, unsigned, unsigned *, unsigned *, unsigned *);
extern int jnox_lifecycle_probe_v1(void), jnox_lifecycle_open_v1(void);
extern int jnox_lifecycle_close_v1(void), jnox_lifecycle_remove_v1(void);
extern unsigned long jnox_packet_token(void), jnox_rx_posted_address_v1(unsigned);
extern unsigned int jnox_lifecycle_page_refs_v1(void);
extern int jnox_original_rx_read(void *,unsigned);

volatile uint64_t jnox_failure_driver[5][32];
unsigned long long jnox_failure_registry[5][3][JN_NETDEV_STAT_COUNT];
static unsigned char frame[1514];

static int hold(volatile uint64_t *row, unsigned phase)
{
    struct timespec target;
    row[1]=phase;
    if (clock_gettime(CLOCK_MONOTONIC,&target)) return 1;
    ++target.tv_sec;
    return clock_nanosleep(CLOCK_MONOTONIC,TIMER_ABSTIME,&target,0)!=0;
}

static int negotiate(uint64_t *mac)
{
    long low=native_call(157,0,0,0), high=native_call(157,1,0,0);
    if (low<0 || high<0 || !(low&32) || !(high&1)) return 10;
    for (unsigned i=0; i<6; ++i) {
        long b=native_call(158,i,1,0);
        if (b<0 || b>255) return 11;
        *mac=(*mac<<8)|(unsigned)b;
    }
    unsigned mtu=0, reads=0, diagnostics=0;
    unsigned long long mask=(1ULL<<32)|((unsigned)low&40u);
    if (jnox_net_validate(&mask,0,&mtu,&reads,&diagnostics) ||
        diagnostics || reads!=!!(low&8) ||
        mask!=((1ULL<<32)|32|((low&8)&&mtu>=68 ? 8 : 0))) return 12;
    return native_call(159,(uint32_t)mask,mask>>32,0) ? 13 : 0;
}

static unsigned posted(void)
{
    unsigned n=0;
    for (unsigned i=0; i<32; ++i) n+=jnox_rx_posted_address_v1(i)!=0;
    return n;
}

static int check_fault(unsigned id)
{
    const volatile unsigned long long *event=jnox_lifecycle_fault_events[id-1];
    const volatile unsigned long long *f=jnox_lifecycle_fault_state[id-1][JF_FIRED];
    const volatile unsigned long long *r=jnox_lifecycle_fault_state[id-1][JF_RETURNED];
    if (event[JF_FIRE_COUNT]!=1 || event[JF_FIRED_ID]!=id || !event[JF_CALLER_PC] ||
        event[JF_ADAPTER_RESULT]!=(id==JF_ALLOC ? 0 : (uint64_t)-12L)) return 34;
    if (!!f[JF_ALLOCATED_DEV]!=(id>=JF_REGISTER) ||
        !!f[JF_REGISTERED_DEV]!=(id>=JF_CPU_DEAD) ||
        !!f[JF_ONLINE_NODE]!=(id>=JF_CPU_DEAD) ||
        !!f[JF_DEAD_NODE]!=(id==JF_XDP_MODEL) ||
        !!f[JF_REGISTERED_RXQ]!=(id==JF_XDP_MODEL) || f[JF_SHARED_MODEL]) return 35;
    if (r[JF_RTNL_OWNER] || r[JF_REGISTERED_RXQ] || r[JF_SHARED_MODEL] ||
        !!r[JF_ONLINE_NODE]!=(id==JF_XDP_MODEL) ||
        !!r[JF_DEAD_NODE]!=(id==JF_XDP_MODEL)) return 36;
    return 0;
}

static int recovery_wire(volatile uint64_t *d, uint64_t mac)
{
    MARK("NET FAILURE RECOVERY READY");
    for (unsigned i=0; i<2; ++i) {
        int n;
        do { n=jnox_original_rx_read(frame,sizeof(frame)); } while (n==-11);
        if (n!=ACK_SIZE) return 40;
        if (!i) d[24]=read_be(frame+22,8);
        if (!(i ? valid_ack(frame,n,mac,d[24]) : valid_frame(frame,n,0,mac,d[24])) ||
            pipe_send(DATA_FD,frame,n) || expect_reply(i ? 0xa7 : 0xa6)) return 41;
        ++d[25];
        if (!i) MARK("NET FAILURE RECOVERY ACK");
    }
    return 0;
}

static int run(unsigned id)
{
    volatile uint64_t *d=jnox_failure_driver[id-1];
    unsigned long long (*net)[JN_NETDEV_STAT_COUNT]=jnox_failure_registry[id-1];
    uint64_t mac=0;
    d[0]=id;
    if (native_call(156,1,0,0) || jnox_packet_context_enter_v1(0)) return 14;
    int rc=negotiate(&mac);
    jnox_packet_context_leave_v1();
    if (rc) return rc;
    jnox_lifecycle_netdev_stats_v1(net[0]);
    if (id<=4 && jnox_lifecycle_fault_arm_v1(id)) return 15;
    if (hold(d,1)) return 16;
    long probe=jnox_lifecycle_probe_v1();
    d[2]=probe;
    if (probe!=(id<4 ? -12 : 0)) return 17;
    if (id>=4) {
        d[3]=jnox_lifecycle_open_v1();
        if (d[3]!=(id==4 ? (uint64_t)-12L : 0)) return 18;
    }
    if (id<=4) {
        if (jnox_lifecycle_fault_snapshot_v1(id,JF_RETURNED)) return 19;
        rc=check_fault(id);
        if (rc) return rc;
    }
    d[4]=id<=4 ? jnox_lifecycle_fault_state[id-1][JF_FIRED][JF_QUEUE_TOKEN] : jnox_packet_token();
    d[5]=jnox_packet_token();
    d[6]=native_call(160,0,0,0);
    d[7]=posted();
    d[8]=jnox_lifecycle_page_refs_v1();
    jnox_lifecycle_netdev_stats_v1(net[1]);
    if (id<4) {
        if (d[5] || d[6]!=(uint64_t)-19L || d[7] || d[8] ||
            net[1][JN_REGISTRY] || net[1][JN_UP] || (!!d[4] != (id!=1))) return 20;
        if (net[1][JN_ALLOC]-net[0][JN_ALLOC]!=(id!=1) ||
            net[1][JN_FREE]-net[0][JN_FREE]!=(id!=1) ||
            net[1][JN_REGISTER]-net[0][JN_REGISTER]!=(id==3) ||
            net[1][JN_UNREGISTER]-net[0][JN_UNREGISTER]!=(id==3)) return 21;
    } else {
        if (!d[5] || d[4]!=d[5] || d[6]!=15 || d[7]!=32 || !d[8] ||
            !net[1][JN_REGISTRY] || !!net[1][JN_UP]!=(id==5)) return 22;
        d[9]=native_call(175,d[5],19,0);
        if (d[9]!=d[5]) return 23;
        if (id==4) {
            d[10]=jnox_original_rx_read(frame,sizeof(frame));
            if (d[10]!=(uint64_t)-100L) return 24;
        }
    }
    struct lifecycle_session s={id,d[4],mac};
    if (pipe_send(DATA_FD,&s,sizeof(s)) || expect_reply(0xa3)) return 25;
    if (hold(d,2)) return 26;
    if (id==5) {
        for (unsigned i=1; i<=3; ++i) {
            uint64_t old=jnox_failure_driver[i][4];
            d[19+i]=native_call(162,old,0,0);
            if (!old || old==d[5] || d[19+i]!=(uint64_t)-116L) return 27;
        }
        rc=recovery_wire(d,mac);
        if (rc) return rc;
        d[11]=jnox_lifecycle_close_v1();
        if (d[11]) return 28;
    }
    if (id>=4) {
        d[12]=jnox_lifecycle_remove_v1();
        if (d[12]) return 29;
    }
    if (id<=4 && jnox_lifecycle_fault_snapshot_v1(id,JF_CLEANED)) return 30;
    jnox_lifecycle_netdev_stats_v1(net[2]);
    d[13]=native_call(160,0,0,0);
    d[14]=jnox_packet_token();
    d[15]=posted();
    d[16]=jnox_lifecycle_page_refs_v1();
    if (d[13]!=(uint64_t)-19L || d[14] || d[15] || d[16] ||
        net[2][JN_REGISTRY] || net[2][JN_UP]) return 31;
    if (d[4]) {
        d[17]=native_call(162,d[4],0,0);
        d[18]=native_call(175,d[4],0,0);
        if (d[17]!=(uint64_t)-116L || d[18]!=(uint64_t)-116L) return 32;
    }
    if (hold(d,3)) return 33;
    d[1]=100;
    return 0;
}

int main(void)
{
    for (unsigned id=1; id<=5; ++id) {
        int rc=run(id);
        jnox_failure_driver[id-1][31]=rc;
        if (rc) return rc;
    }
    MARK("NET FAILURE DRIVER PASS");
    return 0;
}
