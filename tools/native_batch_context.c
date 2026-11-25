/* Optional observer for the two pinned #342 ELFs. Build with QEMU 6.2's
 * official include/qemu/qemu-plugin.h. CPU0 alone writes bounded JSONL.
 * The only data mapping is the explicitly supplied read-only guest RAM file. */
#define _GNU_SOURCE
#include <qemu-plugin.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;
#define RAM_BASE UINT64_C(0x40000000)
#define RAM_SIZE (128UL << 20)
enum point { STORE, AFTER, SAVE_BASE, SAVE_DONE, RESTORE, ERET, IDLE_IRQ, FINISH, POINTS };
struct site { uint64_t pc; uint32_t word; };
static struct site sites[POINTS];
static uint64_t image_base, stack_lo, stack_hi, progress_pa, save_pa;
static const unsigned char *ram;
static FILE *output;
static unsigned records, dropped, invalid, restores;
static uint64_t count;
static bool first, post, user_irq, await_eret, returned, terminal, store_pending;
static bool bad_site[POINTS];
static bool disabled_ram;

static uint64_t word_at(uint64_t pa, unsigned size)
{
    uint64_t value = 0;
    const volatile unsigned char *p = ram + (pa - RAM_BASE);
    for (unsigned i = 0; i < size; ++i)
        value |= (uint64_t)p[i] << (8 * i);
    return value;
}

/* Layout is immutable per package. Recheck once A has actually been loaded.
 * Ignore unrelated code translated at the same user VA in other ELFs. */
static bool memory_ok(void)
{
    if (!ram || disabled_ram)
        return false;
    if (word_at(sites[SAVE_BASE].pc, 4) != sites[SAVE_BASE].word ||
        word_at(sites[SAVE_DONE].pc, 4) != sites[SAVE_DONE].word ||
        word_at(sites[FINISH].pc, 4) != sites[FINISH].word ||
        word_at(image_base + sites[STORE].pc - 0x400000, 4) != sites[STORE].word) {
        disabled_ram = true;
        ++invalid;
        return false;
    }
    return true;
}

static void emit(const char *kind, uint64_t pc, uint64_t pa, bool frame)
{
    if (records >= 63 && strcmp(kind, "terminal")) { ++dropped; return; }
    bool readable = memory_ok();
    fprintf(output, "{\"seq\":%u,\"kind\":\"%s\",\"pc\":%" PRIu64 ",\"pa\":%" PRIu64
            ",\"count\":%" PRIu64 ",\"value\":", records++, kind, pc, pa, count);
    if (readable && progress_pa == image_base + 0x3010)
        fprintf(output, "%" PRIu64 ",\"sum\":%" PRIu64, word_at(progress_pa, 4), word_at(progress_pa - 8, 8));
    else
        fputs("null,\"sum\":null", output);
    fputs(",\"frame\":", output);
    if (frame && readable && pa >= stack_lo && pa <= stack_hi - 832 && !(pa & 15)) {
        fputc('[', output);
        for (unsigned i = 0; i < 35; ++i)
            fprintf(output, "%s%" PRIu64, i ? "," : "", word_at(pa + 8 * i, 8));
        fputc(']', output);
    } else {
        fputs("null", output);
    }
    fprintf(output, ",\"dropped\":%u,\"invalid\":%u}\n", dropped, invalid);
    fflush(output);
}

static void memory(unsigned cpu, qemu_plugin_meminfo_t info, uint64_t va, void *opaque)
{
    enum point point = (enum point)(uintptr_t)opaque;
    if (cpu || terminal) return;
    struct qemu_plugin_hwaddr *hw = qemu_plugin_get_hwaddr(info, va);
    if (!hw || qemu_plugin_hwaddr_is_io(hw)) { ++invalid; return; }
    uint64_t pa = qemu_plugin_hwaddr_phys_addr(hw);
    if (point == STORE) {
        if (va != 0x403010 || pa != image_base + 0x3010 ||
            !qemu_plugin_mem_is_store(info) || qemu_plugin_mem_size_shift(info) != 2) {
            ++invalid;
            return;
        }
        progress_pa = pa;
        ++count;
        store_pending = true;
        return;
    }
    /* A pair may be reported as two 8-byte accesses. Consume only its
     * aligned first word and never follow a guest/user-provided pointer. */
    if ((va & 15) == 8) return;
    unsigned shift = qemu_plugin_mem_size_shift(info);
    if (shift < 3 || shift > 4 || va != pa || (pa & 15) ||
        pa < stack_lo || pa > stack_hi - 832 ||
        qemu_plugin_mem_is_store(info) != (point == SAVE_BASE)) {
        ++invalid;
        return;
    }
    if (point == SAVE_BASE) {
        save_pa = pa;
    } else if (user_irq && restores < 2) {
        ++restores;
        await_eret = true;
        emit("restore", sites[RESTORE].pc, pa, true);
    }
}

static void execute(unsigned cpu, void *opaque)
{
    enum point point = (enum point)(uintptr_t)opaque;
    if (cpu || terminal) return;
    uint64_t pc = sites[point].pc;
    switch (point) {
    case AFTER:
        if (!store_pending) break;
        store_pending = false;
        if (!first) {
            first = true;
            emit("first", pc, progress_pa, false);
        }
        if (returned && !post) {
            post = true;
            emit("post-progress", pc, progress_pa, false);
        }
        break;
    case SAVE_DONE:
        user_irq = true;
        restores = 0;
        returned = post = false;
        emit("irq-save", pc, save_pa, true);
        save_pa = 0;
        break;
    case ERET:
        if (await_eret) {
            await_eret = false;
            returned = true;
            emit("eret", pc, 0, false);
        }
        break;
    case IDLE_IRQ:
        emit("idle-irq", pc, 0, false);
        break;
    case FINISH:
        terminal = true;
        emit("terminal", pc, 0, false);
        break;
    default:
        break;
    }
}

static void translate(qemu_plugin_id_t id, struct qemu_plugin_tb *tb)
{
    (void)id;
    for (size_t i = 0; i < qemu_plugin_tb_n_insns(tb); ++i) {
        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb, i);
        uint64_t pc = qemu_plugin_insn_vaddr(insn);
        for (unsigned p = 0; p < POINTS; ++p) {
            if (pc != sites[p].pc) continue;
            if (qemu_plugin_insn_size(insn) != 4 ||
                memcmp(qemu_plugin_insn_data(insn), &sites[p].word, 4)) {
                if (pc >= RAM_BASE && !bad_site[p]) {
                    bad_site[p] = true;
                    ++invalid;
                }
                continue;
            }
            if (p == STORE || p == SAVE_BASE || p == RESTORE)
                qemu_plugin_register_vcpu_mem_cb(insn, memory, QEMU_PLUGIN_CB_NO_REGS,
                    p == RESTORE ? QEMU_PLUGIN_MEM_R : QEMU_PLUGIN_MEM_W, (void *)(uintptr_t)p);
            else
                qemu_plugin_register_vcpu_insn_exec_cb(insn, execute, QEMU_PLUGIN_CB_NO_REGS,
                    (void *)(uintptr_t)p);
        }
    }
}

static int map_guest(const char *path)
{
    struct stat st, parent;
    const char *slash = strrchr(path, '/');
    if (path[0] != '/' || !slash || slash == path) return -1;
    char *directory = strndup(path, (size_t)(slash - path));
    if (!directory) return -1;
    int dir = open(directory, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC | O_RDONLY);
    free(directory);
    if (dir < 0) return -1;
    int fd = -1;
    if (!fstat(dir, &parent) && parent.st_uid == geteuid() && (parent.st_mode & 0777) == 0700)
        fd = openat(dir, slash + 1, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
    close(dir);
    if (fd < 0) return -1;
    bool valid = !fstat(fd, &st) && S_ISREG(st.st_mode) && st.st_uid == geteuid() &&
                 (st.st_mode & 0777) == 0600 && st.st_nlink == 1 && st.st_size == RAM_SIZE;
    if (valid) {
        void *map = mmap(NULL, RAM_SIZE, PROT_READ, MAP_SHARED, fd, 0);
        if (map != MAP_FAILED) {
            ram = map;
            fprintf(output, "{\"seq\":%u,\"kind\":\"mapping\",\"dev\":%" PRIuMAX
                    ",\"ino\":%" PRIuMAX ",\"bytes\":%lu}\n",
                    records++, (uintmax_t)st.st_dev, (uintmax_t)st.st_ino, RAM_SIZE);
            fflush(output);
        }
    }
    close(fd);
    return ram ? 0 : -1;
}

QEMU_PLUGIN_EXPORT int qemu_plugin_install(qemu_plugin_id_t id, const qemu_info_t *info,
                                         int argc, char **argv)
{
    if (!info->system_emulation || strcmp(info->target_name, "aarch64") ||
        info->system.smp_vcpus != 1) return -1;
    const char *name = NULL, *path = NULL, *file = NULL;
    for (int i = 0; i < argc; ++i) {
        if (!strncmp(argv[i], "case=", 5) && !name) name = argv[i] + 5;
        else if (!strncmp(argv[i], "out=", 4) && !path) path = argv[i] + 4;
        else if (!strncmp(argv[i], "ram=", 4) && !file) file = argv[i] + 4;
        else return -1;
    }
    if (!name || !path || path[0] != '/') return -1;
    bool arrival = !strcmp(name, "arrival-srtf");
    if (!arrival && strcmp(name, "tie-sjf")) return -1;
    image_base = arrival ? 0x4028c000 : 0x402a1000;
    stack_lo = arrival ? 0x40ab1000 : 0x40edb000;
    stack_hi = stack_lo + 0x4000;
    sites[STORE] = arrival ? (struct site){0x400a3c, 0xb90012d5} : (struct site){0x400960, 0xb9001314};
    sites[AFTER] = arrival ? (struct site){0x400a40, 0x110006b5} : (struct site){0x400964, 0x97fffe48};
    sites[SAVE_BASE] = (struct site){0x402011c8, 0xa90007e0};
    sites[SAVE_DONE] = (struct site){0x40201290, arrival ? 0x940004ab : 0x9400049f};
    sites[RESTORE] = (struct site){0x402013e8, 0xa94007e0};
    sites[ERET] = (struct site){0x40201480, 0xd69f03e0};
    sites[IDLE_IRQ] = (struct site){0x402014d8, arrival ? 0x94000419 : 0x9400040d};
    sites[FINISH] = (struct site){arrival ? 0x40203218 : 0x4020323c, 0xd10243ff};
    int fd = open(path, O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC | O_NOFOLLOW, 0600);
    if (fd < 0) return -1;
    output = fdopen(fd, "w");
    if (!output) { close(fd); return -1; }
    if (file && map_guest(file)) {
        fputs("{\"seq\":0,\"kind\":\"mapping-unavailable\"}\n", output);
        ++records;
        fflush(output);
    }
    qemu_plugin_register_vcpu_tb_trans_cb(id, translate);
    return 0;
}
