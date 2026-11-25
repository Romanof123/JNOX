// SPDX-License-Identifier: GPL-2.0-or-later
// QEMU 6.2 full-system Linux host diagnostic, one vCPU. All output is host RAM.
#define _GNU_SOURCE
#include <qemu-plugin.h>
#include <errno.h>
#include <fcntl.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;
enum { REGIONS = 3, CAPACITY = 4096, WORDS = 15 };
struct region { uint64_t call, first, limit; };
static struct region sites[REGIONS];
static uint64_t (*records)[WORDS];
static size_t used;
static _Thread_local uint64_t translations, matched[REGIONS];
static _Thread_local long host_tid;
static bool empty;

static uint64_t ns(clockid_t id)
{
    struct timespec t;
    if (clock_gettime(id, &t)) return UINT64_MAX;
    return (uint64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}

static void hit(unsigned int cpu, void *tag)
{
    if (cpu || used == CAPACITY) {
        records[0][2]++;
        return;
    }
    uint64_t *row = records[1 + used];
    row[0] = ns(CLOCK_MONOTONIC);
    row[1] = ns(CLOCK_THREAD_CPUTIME_ID);
    row[2] = ns(CLOCK_MONOTONIC_RAW);
    row[3] = (uintptr_t)tag;
    if (!host_tid) host_tid = syscall(SYS_gettid);
    row[4] = host_tid;
    row[5] = translations;
    for (size_t i = 0; i < REGIONS; i++) row[6+i] = matched[i];
    struct rusage usage;
    if (getrusage(RUSAGE_THREAD, &usage)) {
        row[9] = row[10] = UINT64_MAX;
    } else {
        row[9] = usage.ru_nvcsw;
        row[10] = usage.ru_nivcsw;
    }
    row[11] = ns(CLOCK_MONOTONIC);
    atomic_thread_fence(memory_order_release);
    records[0][1] = ++used;
}

static void translated(qemu_plugin_id_t id, struct qemu_plugin_tb *tb)
{
    (void)id;
    translations++;
    bool overlaps[REGIONS] = {false};
    for (size_t j = 0; j < qemu_plugin_tb_n_insns(tb); j++) {
        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb,j);
        uint64_t pc = qemu_plugin_insn_vaddr(insn);
        for (size_t i = 0; i < REGIONS; i++) {
            struct region *r = &sites[i];
            if (pc >= r->first && pc < r->limit) overlaps[i] = true;
            if (pc == r->call || pc == r->call + 4) {
                uintptr_t tag = 2*i + (pc != r->call);
                qemu_plugin_register_vcpu_insn_exec_cb(insn, hit,
                    QEMU_PLUGIN_CB_NO_REGS, (void *)tag);
            }
        }
    }
    for (size_t i = 0; i < REGIONS; i++) matched[i] += overlaps[i];
}

QEMU_PLUGIN_EXPORT int qemu_plugin_install(qemu_plugin_id_t id,
        const qemu_info_t *info, int argc, char **argv)
{
    if (argc != 5 || !info->system_emulation || info->system.smp_vcpus != 1 ||
        strcmp(info->target_name,"aarch64")) return -1;
    if (strncmp(argv[0],"out=",4) || !argv[0][4]) return -1;
    if (!strcmp(argv[1],"mode=empty")) empty = true;
    else if (strcmp(argv[1],"mode=observe")) return -1;
    for (size_t i = 0; i < REGIONS; i++) {
        char extra;
        if (sscanf(argv[i+2], "region=%" SCNx64 ":%" SCNx64 ":%" SCNx64 "%c",
                   &sites[i].call, &sites[i].first, &sites[i].limit, &extra) != 3 ||
            !sites[i].call || sites[i].first >= sites[i].limit) return -1;
    }
    int fd = open(argv[0]+4, O_EXCL | O_CREAT | O_RDWR, 0600);
    if (fd < 0) return -1;
    size_t size = (1 + CAPACITY) * sizeof(*records);
    if (ftruncate(fd, size)) { close(fd); return -1; }
    records = mmap(NULL,size,PROT_READ|PROT_WRITE,MAP_SHARED,fd,0);
    close(fd);
    if (records == MAP_FAILED) return -1;
    records[0][0] = 0x4a484f530001;
    if (!empty) qemu_plugin_register_vcpu_tb_trans_cb(id, translated);
    return 0;
}
