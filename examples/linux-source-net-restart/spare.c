#include "driver_common.h"

static int run(void)
{
    struct handoff old;
    if (signal_byte(DATA_FD,0xb2) || pipe_receive(REPLY_FD,&old,sizeof(old)) ||
        !old.old_token) return 30;
    D[10]=old.old_token; D[11]=old.old_nonce;
    int rc=start_device();
    if (rc) return rc;
    if (sess.token == old.old_token) return 31;
    D[12]=native_call(162,old.old_token,0,0);
    D[13]=native_call(171,old.old_token,(uintptr_t)frame,1530);
    D[14]=native_call(172,old.old_token,0,0);
    D[15]=native_call(173,old.old_token,2,0);
    D[16]=native_call(174,old.old_token,0,0);
    D[17]=native_call(175,old.old_token,0,0);
    for (unsigned i=12; i<=17; ++i) if (D[i]!=(uint64_t)-116L) return 32;
    if (pipe_send(DATA_FD,&sess,sizeof(sess)) || await_byte(REPLY_FD,0xa2)) return 33;
    D[1]=2;
    MARK("NET RESTART SPARE READY");
    for (unsigned seq=0; seq<FRAME_COUNT; ++seq) {
        int n=receive_frame();
        if (n != (int)frame_length(seq)) return 34;
        if (!seq) D[5]=read_be(frame+22,8);
        if (D[5]==old.old_nonce || !valid_frame(frame,n,seq,sess.mac,D[5])) return 35;
        if (pipe_send(DATA_FD,frame,n)) return 36;
        ++D[6]; D[7]+=n;
    }
    if (await_byte(REPLY_FD,0xa6) || callbacks(FRAME_COUNT)) return 37;
    D[1]=3;
    MARK("NET RESTART WAIT ACK");
    int n=receive_frame();
    if (n!=ACK_SIZE || !valid_ack(frame,n,sess.mac,D[5]) || pipe_send(DATA_FD,frame,n) ||
        await_byte(REPLY_FD,0xa7) || callbacks(FRAME_COUNT+1)) return 38;
    D[18]=n;
    D[19]=native_call(156,0,0,0);
    if (D[19]) return 39;
    jnox_net_forget_queues();
    D[1]=100;
    return 0;
}
int main(void)
{
    D[0]=0x5245535444525632ULL;
    D[1]=1;
    int rc=run();
    D[2]=rc;
    return rc;
}
