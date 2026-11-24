/* Linux 64-bit, little-endian. Public QEMU 6.2 plugin ABI v1 subset.
 * No QEMU structs are dereferenced; registration uses QEMU_PLUGIN_CB_NO_REGS. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
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
#error "This capture format requires little-endian Linux"
#endif
int qemu_plugin_version = 1;
enum { SITES = 6, CAP = 2048 };
struct capture {
    uint64_t count, invalid, wall_resolution, cpu_resolution;
    struct row { uint64_t site, cpu, tid, wall0, thread, wall1; } rows[CAP];
};
static struct capture *capture;
static struct site { uint64_t pc; uint32_t word; } sites[SITES];

static uint64_t ns(struct timespec t)
{
    return (uint64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}

static void sample(unsigned cpu, void *data)
{
    struct timespec wall0, thread, wall1;
    if (cpu || capture->count >= CAP ||
        clock_gettime(CLOCK_MONOTONIC, &wall0) ||
        clock_gettime(CLOCK_THREAD_CPUTIME_ID, &thread) ||
        clock_gettime(CLOCK_MONOTONIC, &wall1)) {
        capture->invalid = 1;
        return;
    }
    capture->rows[capture->count++] = (struct row){
        (struct site *)data - sites, cpu, gettid(), ns(wall0), ns(thread), ns(wall1)
    };
}

static void translate(qemu_plugin_id_t id, struct qemu_plugin_tb *tb)
{
    (void)id;
    for (size_t i = 0; i < qemu_plugin_tb_n_insns(tb); i++) {
        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb, i);
        uint64_t pc = qemu_plugin_insn_vaddr(insn);
        for (size_t j = 0; j < SITES; j++) {
            if (pc != sites[j].pc) continue;
            if (qemu_plugin_insn_size(insn) != 4 ||
                memcmp(qemu_plugin_insn_data(insn), &sites[j].word, 4)) {
                capture->invalid = 1;
                continue;
            }
            qemu_plugin_register_vcpu_insn_exec_cb(insn, sample, QEMU_PLUGIN_CB_NO_REGS, &sites[j]);
        }
    }
}

int qemu_plugin_install(qemu_plugin_id_t id, const struct qemu_info *info,
                        int argc, char **argv)
{
    (void)info;
    if (argc != SITES + 1) return -1;
    for (size_t i = 0; i < SITES; i++) {
        char extra;
        unsigned long long pc;
        unsigned index, word;
        if (sscanf(argv[i+1], "site%u=%llx:%x%c", &index, &pc, &word, &extra) != 3 || index != i) return -1;
        sites[i] = (struct site){pc, word};
    }
    if (strncmp(argv[0], "out=", 4)) return -1;
    int fd = open(argv[0]+4, O_CREAT | O_EXCL | O_RDWR, 0600);
    if (fd < 0) return -1;
    if (ftruncate(fd, sizeof(*capture))) { close(fd); return -1; }
    capture = mmap(NULL, sizeof(*capture), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (capture == MAP_FAILED) return -1;
    struct timespec w, c;
    if (clock_getres(CLOCK_MONOTONIC, &w) || clock_getres(CLOCK_THREAD_CPUTIME_ID, &c)) return -1;
    capture->wall_resolution = ns(w);
    capture->cpu_resolution = ns(c);
    qemu_plugin_register_vcpu_tb_trans_cb(id, translate);
    return 0;
}
