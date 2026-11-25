#define _GNU_SOURCE
#include <qemu-plugin.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdatomic.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#if QEMU_PLUGIN_VERSION != 7 || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error Frozen Linux little-endian QEMU plugin ABI 7 required
#endif
QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;
enum { CAP = 1024, WORDS = 16, USER_BASE = 0x400000, RESULT = 0x402000,
       VECTOR = 0x2001800, STAMP = 0x2002148, ERET = 0x2002154,
       IDLE_ERET = 0x2002170, LAND = 0x2002174 };
enum { ATTEMPT, USED, DROP, EXEC, MEM, COMPLETE, ABORTED, INVALID, GEN,
       PENDING, WAIT_USER, DISCONS, RETURNS, FIRST, FIRST_DROP, HAVE_MEM };
enum { RETURN = 1, USER, DISCON, VECTOR_ENTRY, STORE, IDLE_RETURN, IDLE_LAND };
struct stream { uint64_t h[16], rows[CAP][WORDS]; };
struct file { uint64_t header[8]; struct stream cpu[2]; };
_Static_assert(sizeof(struct file) == (8 + 2*(16+CAP*WORDS))*8, "wire layout");
static struct file *out;
static struct qemu_plugin_scoreboard *gate;
static const uint64_t store_pc[] = {0x40048c, 0x400084};
static const uint32_t store_word[] = {0xb900012b,0xb900014b};
static const uint32_t next_word[] = {0x1100090b,0x17fffff0};
static const uint64_t pa_result[] = {34824192,39104512};
static struct live {
    bool user, stamped, sampling;
    uint64_t r, next_r, pre_lo, pre_hi, value;
} live[2];

static uint64_t ns(unsigned cpu)
{
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC,&t)) {
        out->cpu[cpu].h[INVALID]++;
        return 0;
    }
    return (uint64_t)t.tv_sec*1000000000u + t.tv_nsec;
}

static void append(unsigned cpu, unsigned kind, uint64_t pc, uint64_t aux,
                   uint64_t va, uint64_t pa, uint64_t value, uint64_t ordinal,
                   uint64_t a, uint64_t b, uint64_t c, uint64_t d, uint64_t extra)
{
    struct stream *s = &out->cpu[cpu];
    uint64_t seq = ++s->h[ATTEMPT];
    if (seq > CAP) {
        if (!s->h[DROP]++) s->h[FIRST_DROP] = seq;
        return;
    }
    uint64_t *row = s->rows[seq-1];
    uint64_t data[15] = {kind,cpu,s->h[GEN],pc,aux,va,pa,value,ordinal,a,b,c,d,live[cpu].r,extra};
    memcpy(row+1,data,sizeof data);
    __atomic_store_n(row,seq,__ATOMIC_RELEASE);
    __atomic_store_n(&s->h[USED],seq,__ATOMIC_RELEASE);
}

static void marker(unsigned cpu, unsigned kind, uint64_t pc, uint64_t aux, uint64_t extra, uint64_t lo)
{
    append(cpu,kind,pc,aux,0,0,0,0,lo,ns(cpu),0,0,extra);
}

static void clear_pending(unsigned cpu)
{
    uint64_t *h = out->cpu[cpu].h;
    if (h[PENDING]) h[ABORTED]++;
    h[PENDING] = h[HAVE_MEM] = 0;
    live[cpu].sampling = false;
}

static void leave(unsigned cpu)
{
    clear_pending(cpu);
    live[cpu].user = false;
    live[cpu].stamped = false;
    out->cpu[cpu].h[WAIT_USER] = 0;
    qemu_plugin_u64_set(qemu_plugin_scoreboard_u64(gate),cpu,0);
}

static void discontinuity(unsigned cpu, enum qemu_plugin_discon_type type,
                          uint64_t from, uint64_t to, void *unused)
{
    (void)unused;
    if (cpu >= 2) return;
    uint64_t lo = ns(cpu);
    leave(cpu);
    out->cpu[cpu].h[DISCONS]++;
    marker(cpu,DISCON,from,to,type,lo);
}

static void boundary(unsigned cpu, void *data)
{
    if (cpu >= 2) return;
    uint64_t lo = ns(cpu), pc = (uintptr_t)data;
    uint64_t *h = out->cpu[cpu].h;
    if (pc == ERET) {
        if (!live[cpu].stamped || h[WAIT_USER] || live[cpu].user) h[INVALID]++;
        uint64_t r = live[cpu].next_r;
        leave(cpu);
        live[cpu].r = r;
        h[GEN]++;
        h[RETURNS]++;
        h[WAIT_USER] = 1;
        qemu_plugin_u64_set(qemu_plugin_scoreboard_u64(gate),cpu,1);
        marker(cpu,RETURN,pc,0,0,lo);
    } else {
        leave(cpu);
        unsigned kind = pc==IDLE_ERET ? IDLE_RETURN : pc==LAND ? IDLE_LAND : VECTOR_ENTRY;
        marker(cpu,kind,pc,0,0,lo);
    }
}

static void first_user(unsigned cpu, void *data)
{
    if (cpu >= 2) return;
    uint64_t lo = ns(cpu);
    uint64_t *h = out->cpu[cpu].h;
    if (!h[WAIT_USER]) { h[INVALID]++; return; }
    qemu_plugin_u64_set(qemu_plugin_scoreboard_u64(gate),cpu,0);
    h[WAIT_USER] = 0;
    h[FIRST]++;
    live[cpu].user = true;
    marker(cpu,USER,(uintptr_t)data,0,0,lo);
}

static void store_before(unsigned cpu, void *data)
{
    if (cpu >= 2 || (uintptr_t)data != store_pc[cpu]) return;
    uint64_t *h = out->cpu[cpu].h;
    if (!live[cpu].user || h[PENDING]) h[INVALID]++;
    clear_pending(cpu);
    h[EXEC]++;
    h[PENDING] = 1;
    live[cpu].sampling = (h[COMPLETE]+1)%256 == 0;
    if (live[cpu].sampling) {
        live[cpu].pre_lo = ns(cpu);
        live[cpu].pre_hi = ns(cpu);
    }
}

static void store_mem(unsigned cpu, qemu_plugin_meminfo_t info, uint64_t va, void *data)
{
    if (cpu >= 2) return;
    uint64_t pc = (uintptr_t)data;
    bool stamp = pc == STAMP;
    if (!stamp && pc != store_pc[cpu]) return;
    uint64_t *h = out->cpu[cpu].h;
    struct qemu_plugin_hwaddr *hw = qemu_plugin_get_hwaddr(info,va);
    uint64_t want_va = stamp ? 0x2028000 + 64*cpu : RESULT;
    uint64_t want_pa = stamp ? want_va : pa_result[cpu];
    if (!hw || va!=want_va || qemu_plugin_hwaddr_is_io(hw) || qemu_plugin_hwaddr_phys_addr(hw)!=want_pa ||
        !qemu_plugin_mem_is_store(info) || qemu_plugin_mem_is_big_endian(info) ||
        qemu_plugin_mem_size_shift(info)!=(stamp ? 3u : 2u)) { h[INVALID]++; return; }
    qemu_plugin_mem_value val = qemu_plugin_mem_get_value(info);
    if (stamp) {
        if (val.type!=QEMU_PLUGIN_MEM_VALUE_U64 || !val.data.u64 || live[cpu].stamped) {
            h[INVALID]++; return;
        }
        live[cpu].stamped = true;
        live[cpu].next_r = val.data.u64;
    } else {
        if (val.type!=QEMU_PLUGIN_MEM_VALUE_U32 || !live[cpu].user || !h[PENDING] || h[HAVE_MEM]) {
            h[INVALID]++; return;
        }
        h[MEM]++;
        h[HAVE_MEM] = 1;
        live[cpu].value = val.data.u32;
        if (live[cpu].value != h[MEM]+(cpu==0)) h[INVALID]++;
    }
}

static void fallthrough(unsigned cpu, void *data)
{
    if (cpu>=2 || (uintptr_t)data!=store_pc[cpu]+4) return;
    uint64_t *h=out->cpu[cpu].h;
    if (!h[PENDING]) return;  // A also branches here for a composite candidate
    uint64_t lo = live[cpu].sampling ? ns(cpu) : 0;
    if (!h[HAVE_MEM] || !live[cpu].user) {
        h[INVALID]++;
        clear_pending(cpu);
        return;
    }
    h[COMPLETE]++;
    if (live[cpu].sampling)
        append(cpu,STORE,store_pc[cpu],store_pc[cpu]+4,RESULT,pa_result[cpu],
               live[cpu].value,h[COMPLETE],live[cpu].pre_lo,live[cpu].pre_hi,lo,ns(cpu),0);
    h[PENDING] = h[HAVE_MEM] = 0;
    live[cpu].sampling = false;
}

static bool word(struct qemu_plugin_insn *insn, uint32_t wanted)
{
    uint32_t w;
    return qemu_plugin_insn_size(insn)==4 && qemu_plugin_insn_data(insn,&w,4)==4 && w==wanted;
}

static void translate(struct qemu_plugin_tb *tb, void *unused)
{
    (void)unused;
    for (size_t i=0; i<qemu_plugin_tb_n_insns(tb); ++i) {
        struct qemu_plugin_insn *insn=qemu_plugin_tb_get_insn(tb,i);
        uint64_t pc=qemu_plugin_insn_vaddr(insn);
        void *tag=(void *)(uintptr_t)pc;
        if (i==0 && pc>=USER_BASE && pc<RESULT)
            qemu_plugin_register_vcpu_insn_exec_cond_cb(insn,first_user,QEMU_PLUGIN_CB_NO_REGS,
                QEMU_PLUGIN_COND_EQ,qemu_plugin_scoreboard_u64(gate),1,tag);
        if ((pc>=VECTOR && pc<VECTOR+2048 && (pc-VECTOR)%128==0) ||
            ((pc==ERET || pc==IDLE_ERET) && word(insn,0xd69f03e0)) ||
            (pc==LAND && word(insn,0x58001921)))
            qemu_plugin_register_vcpu_insn_exec_cb(insn,boundary,QEMU_PLUGIN_CB_NO_REGS,tag);
        if (pc==STAMP && word(insn,0xc89ffe11))
            qemu_plugin_register_vcpu_mem_cb(insn,store_mem,QEMU_PLUGIN_CB_NO_REGS,QEMU_PLUGIN_MEM_W,tag);
        for (unsigned c=0;c<2;c++) {
            if (pc==store_pc[c] && word(insn,store_word[c])) {
                qemu_plugin_register_vcpu_insn_exec_cb(insn,store_before,QEMU_PLUGIN_CB_NO_REGS,tag);
                qemu_plugin_register_vcpu_mem_cb(insn,store_mem,QEMU_PLUGIN_CB_NO_REGS,QEMU_PLUGIN_MEM_W,tag);
            } else if (pc==store_pc[c]+4 && word(insn,next_word[c])) {
                qemu_plugin_register_vcpu_insn_exec_cb(insn,fallthrough,QEMU_PLUGIN_CB_NO_REGS,tag);
            }
        }
    }
}

QEMU_PLUGIN_EXPORT int qemu_plugin_install(qemu_plugin_id_t id,const qemu_info_t *info,int argc,char **argv)
{
    if (argc!=1 || strncmp(argv[0],"out=/",5) || !info->system_emulation ||
        strcmp(info->target_name,"aarch64") || info->system.smp_vcpus!=4 ||
        info->version.cur!=7) return -1;
    struct timespec resolution;
    if (clock_getres(CLOCK_MONOTONIC,&resolution)) return -1;
    gate=qemu_plugin_scoreboard_new(sizeof(uint64_t));
    if (!gate) return -1;
    int fd=open(argv[0]+4,O_RDWR|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);
    if (fd<0) return -1;
    if (posix_fallocate(fd,0,sizeof *out)) { close(fd); return -1; }
    out=mmap(NULL,sizeof *out,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
    close(fd);
    if (out==MAP_FAILED) return -1;
    memset(out,0,sizeof *out);
    uint64_t hdr[]={0x3535334f425331ULL,1,2,CAP,WORDS,
                    (uint64_t)resolution.tv_sec*1000000000u+resolution.tv_nsec,7};
    memcpy(out->header,hdr,sizeof hdr);
    qemu_plugin_register_vcpu_discon_cb(id,QEMU_PLUGIN_DISCON_ALL,discontinuity,NULL);
    qemu_plugin_register_vcpu_tb_trans_cb(id,translate,NULL);
    __atomic_store_n(out->header+7,1,__ATOMIC_RELEASE);
    return 0;
}
