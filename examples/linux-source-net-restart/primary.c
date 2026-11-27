#include "driver_common.h"

static int run(void)
{
    int rc = start_device();
    if (rc) return rc;
    if (pipe_send(DATA_FD,&sess,sizeof(sess)) || await_byte(REPLY_FD,0xa1)) return 20;
    D[1]=2;
    MARK("NET RESTART PRIMARY READY");
    int n = receive_frame();
    if (n != (int)frame_length(0)) return 21;
    uint64_t nonce=read_be(frame+22,8);
    if (!valid_frame(frame,n,0,sess.mac,nonce)) return 22;
    D[5]=nonce; D[6]=1; D[7]=n;
    if (callbacks(1) || pipe_send(DATA_FD,frame,n)) return 23;
    D[1]=3;
    if (await_byte(REPLY_FD,0xe1)) return 24;
    D[1]=86;
    /* CRT performs SVC 1 with this status. Leave all outstanding grants live. */
    return 86;
}
int main(void)
{
    D[0]=0x5245535444525630ULL;
    D[1]=1;
    int rc=run();
    D[2]=rc;
    return rc;
}
