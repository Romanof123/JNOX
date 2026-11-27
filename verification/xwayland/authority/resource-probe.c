/* Independent native SVC 199 reader. Only a bounded case code leaves this process. */
#include <jnox/ipc.h>
#include <stddef.h>
#include <stdint.h>

enum { OWN = 40, OTHER = 41, MISSING = 42, INVALID = 43, DENIED = 44,
       DISABLED = 45, ONE = 46, MAXIMUM = 47 };
static unsigned char buf[258], repeat[256];
static long call(unsigned op, uintptr_t ptr, size_t cap)
{
    return jnox_ipc_trap(199, op, ptr, cap).status;
}
static int equal(const unsigned char *a, const unsigned char *b, size_t n)
{
    for (size_t i=0; i<n; i++) if (a[i]!=b[i]) return 0;
    return 1;
}
static int field(size_t *pos, size_t size, const char *value, size_t n)
{
    if (*pos+2+n>size || buf[1+*pos]!=0 || buf[2+*pos]!=n) return 0;
    *pos+=2;
    if (value && !equal(buf+1+*pos,(const unsigned char *)value,n)) return 0;
    *pos+=n;
    return 1;
}
static int examine(char role)
{
    long size=call(0,0,0);
    for (size_t i=0;i<sizeof(buf);i++) buf[i]=0xa5;
    uintptr_t dest=(uintptr_t)(buf+1);
    if (size<0) {
        if (call(1,dest,256)!=size) return 80;
        for (size_t i=0;i<sizeof(buf);i++) if (buf[i]!=0xa5) return 81;
        if (size==-38) return DISABLED;
        if (role!='S' && role!='C')
            return size==-13 && call(2,0,0)==-13 ? DENIED : 82;
        return size==-2 ? MISSING : size==-22 ? INVALID : 83;
    }
    if ((role!='S' && role!='C') || size<1 || size>256) return 84;
    if (call(0,dest,0)!=-22 || call(0,0,1)!=-22 || call(2,0,0)!=-22) return 85;
    if (call(1,dest,size-1)!=-34 || call(1,0,size)!=-14) return 86;
    for (size_t i=0;i<sizeof(buf);i++) if (buf[i]!=0xa5) return 87;
    if (size>8) {
        /* Last eight mapped bytes of the fixed ordinary stack, then its guard. */
        volatile unsigned char *tail=(volatile unsigned char *)(uintptr_t)0x01010ff8;
        unsigned char saved[8];
        for (unsigned i=0;i<8;i++) { saved[i]=tail[i]; tail[i]=0xa5; }
        long fault=call(1,(uintptr_t)tail,size);
        int intact=1;
        for (unsigned i=0;i<8;i++) { if (tail[i]!=0xa5) intact=0; tail[i]=saved[i]; }
        if (fault!=-14 || !intact) return 88;
    }
    if (call(1,dest,256)!=size || call(1,(uintptr_t)repeat,size)!=size ||
        !equal(buf+1,repeat,size) || call(0,0,0)!=size) return 89;
    if (buf[0]!=0xa5) return 90;
    for (size_t i=(size_t)size+1;i<sizeof(buf);i++) if (buf[i]!=0xa5) return 91;
    if (size==1 || size==256) return size==1 ? ONE : MAXIMUM;
    if (buf[1]!=1 || buf[2]!=0) return 92; /* FamilyLocal, network order */
    size_t pos=2;
    int own=field(&pos,size,role=='S' ? "srv" : "cli",3);
    if (!own) {
        pos=2;
        if (!field(&pos,size,role=='S' ? "cli" : "srv",3)) return 93;
    }
    if (!field(&pos,size,"0",1) || !field(&pos,size,"MIT-MAGIC-COOKIE-1",18) ||
        !field(&pos,size,NULL,16) || pos!=(size_t)size) return 94;
    return own ? OWN : OTHER;
}
int main(int argc,char **argv)
{
    return examine(argc==2 ? argv[1][0] : 'D');
}
