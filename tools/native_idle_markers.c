/* Same no-register public QEMU 6.2 ABI v1 used by #178 native_weighted_marker.c.
 * Single Linux vCPU. All file storage is reserved before any execution callback. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

typedef uint64_t qemu_plugin_id_t;
struct qemu_plugin_tb;
struct qemu_plugin_insn;
struct qemu_info;
typedef enum { QEMU_PLUGIN_CB_NO_REGS } qemu_plugin_cb_flags;
extern size_t qemu_plugin_tb_n_insns(const struct qemu_plugin_tb *);
extern struct qemu_plugin_insn *qemu_plugin_tb_get_insn(const struct qemu_plugin_tb *, size_t);
extern uint64_t qemu_plugin_insn_vaddr(const struct qemu_plugin_insn *);
extern size_t qemu_plugin_insn_size(const struct qemu_plugin_insn *);
extern const void *qemu_plugin_insn_data(const struct qemu_plugin_insn *);
extern void qemu_plugin_register_vcpu_tb_trans_cb(qemu_plugin_id_t,
    void (*)(qemu_plugin_id_t, struct qemu_plugin_tb *));
extern void qemu_plugin_register_vcpu_insn_exec_cb(struct qemu_plugin_insn *,
    void (*)(unsigned, void *), qemu_plugin_cb_flags, void *);

#if !defined(__linux__) || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error "Requires little-endian Linux"
#endif
int qemu_plugin_version = 1;
enum { CAP = 8192, SITES = 4, CAL = 32 };
static const struct site { uint64_t pc; uint32_t word; } sites[SITES] = {
    {0x4020148c, 0xd53be021}, {0x40201490, 0xa9000801},
    {0x402014ac, 0xd53be021}, {0x402014b0, 0x94000412}
};
struct record { uint64_t seq, site, cpu, tid, wall0, thread, wall1; };
static struct log {
    uint64_t magic, version, count, dropped, invalid, wall_res, cpu_res;
    uint64_t calibration[CAL];
    struct record rows[CAP];
} *logbuf;

static uint64_t ns(struct timespec t) {
    return (uint64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}

static int clocks(struct record *r) {
    struct timespec a, c, b;
    if (clock_gettime(CLOCK_MONOTONIC, &a) || clock_gettime(CLOCK_THREAD_CPUTIME_ID, &c)
        || clock_gettime(CLOCK_MONOTONIC, &b)) return -1;
    r->wall0 = ns(a); r->thread = ns(c); r->wall1 = ns(b);
    return r->wall1 < r->wall0 ? -1 : 0;
}

static void sample(unsigned cpu, void *data) {
    if (cpu) { __atomic_fetch_or(&logbuf->invalid, 1, __ATOMIC_RELAXED); return; }
    uint64_t i = logbuf->count;
    if (i == CAP) {
        if (logbuf->dropped != UINT64_MAX) ++logbuf->dropped;
        return;
    }
    struct record r = {.seq=i, .site=(const struct site *)data-sites, .cpu=cpu, .tid=gettid()};
    if (clocks(&r)) __atomic_fetch_or(&logbuf->invalid, 2, __ATOMIC_RELAXED);
    logbuf->rows[i] = r;
    __atomic_store_n(&logbuf->count, i+1, __ATOMIC_RELEASE);
}

static void translate(qemu_plugin_id_t id, struct qemu_plugin_tb *tb) {
    (void)id;
    for (size_t i=0; i<qemu_plugin_tb_n_insns(tb); ++i) {
        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb,i);
        uint64_t pc = qemu_plugin_insn_vaddr(insn);
        for (size_t j=0; j<SITES; ++j) {
            if (pc != sites[j].pc) continue;
            if (qemu_plugin_insn_size(insn) != 4 ||
                memcmp(qemu_plugin_insn_data(insn), &sites[j].word, 4)) {
                __atomic_fetch_or(&logbuf->invalid, 4, __ATOMIC_RELAXED);
                continue;
            }
            qemu_plugin_register_vcpu_insn_exec_cb(insn, sample, QEMU_PLUGIN_CB_NO_REGS, (void *)&sites[j]);
        }
    }
}

int qemu_plugin_install(qemu_plugin_id_t id, const struct qemu_info *info, int argc, char **argv) {
    (void)info;
    if (argc != SITES+1 || strncmp(argv[0],"out=",4)) return -1;
    for (unsigned j=0; j<SITES; ++j) {
        char expected[64];
        snprintf(expected,sizeof expected,"site%u=%llx:%x",j,(unsigned long long)sites[j].pc,sites[j].word);
        if (strcmp(argv[j+1],expected)) return -1;
    }
    int fd = open(argv[0]+4,O_RDWR|O_CREAT|O_EXCL,0600);
    if (fd < 0) return -1;
    if (posix_fallocate(fd,0,sizeof *logbuf)) { close(fd); return -1; }
    logbuf = mmap(NULL,sizeof *logbuf,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
    close(fd);
    if (logbuf == MAP_FAILED) return -1;
    struct timespec w,c;
    if (clock_getres(CLOCK_MONOTONIC,&w) || clock_getres(CLOCK_THREAD_CPUTIME_ID,&c)) return -1;
    logbuf->magic = 0x393333584f4e4aULL;
    logbuf->version = 1;
    logbuf->wall_res = ns(w); logbuf->cpu_res = ns(c);
    for (unsigned i=0; i<CAL; ++i) {
        struct record r;
        if (clocks(&r)) return -1;
        logbuf->calibration[i] = r.wall1-r.wall0;
    }
    qemu_plugin_register_vcpu_tb_trans_cb(id,translate);
    return 0;
}
