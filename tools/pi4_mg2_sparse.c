// Fixed clustered MG2 instrument. All guest reads are from linked ordinary RAM.
#include <glib.h>
#include <qemu-plugin.h>
#include <stdatomic.h>
#include <stdint.h>
#include <string.h>

QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;

enum { LOG_BASE = 0xa9178, COUNT = LOG_BASE + 6912, SGIR = 0xff841f00u };
struct site { uint64_t pc; uint32_t word; unsigned cpu; };
static struct site sites[] = {{0x81a00, 0xc89ffd28, 0}, {0x83954, 0xb9000100, 1}};
static atomic_bool complete[2], invalid;
static bool peer[2], armed[2];
static uint64_t last_count, dispatch[3];

static void reject(void)
{
    if (!atomic_exchange(&invalid, true)) {
        qemu_plugin_outs("MG2SP INVALID\n");
    }
}

static bool ram(uint64_t addr, uint64_t *words, unsigned count)
{
    GByteArray *bytes = g_byte_array_sized_new(count * 8);
    bool ok = qemu_plugin_read_memory_vaddr(addr, bytes, count * 8);
    if (ok && bytes->len == count * 8) {
        memcpy(words, bytes->data, count * 8);
        for (unsigned i = 0; i < count; i++) {
            words[i] = GUINT64_FROM_LE(words[i]);
        }
    } else {
        ok = false;
    }
    g_byte_array_unref(bytes);
    return ok;
}

static void before(unsigned cpu, void *data)
{
    struct site *s = data;
    if (cpu == s->cpu) {
        if (armed[cpu]) {
            reject();
        }
        armed[cpu] = true;
        peer[cpu] = atomic_load_explicit(&complete[1-cpu], memory_order_acquire);
    }
}

static bool final_row(uint64_t count, uint64_t *lease)
{
    uint64_t used, w[9];
    if (count != last_count + 1 || count > 96 || !ram(COUNT, &used, 1) ||
        used != count || !ram(LOG_BASE + (count - 1) * 72, w, 9)) {
        reject();
        return false;
    }
    last_count = count;
    if (w[8] == 11) {
        memcpy(dispatch, w, sizeof(dispatch));
    }
    if (w[8] != 5 || w[0] != 0 || w[3] != 65536) {
        return false;
    }
    uint64_t live, task_lease;
    if (!ram(0x8d130, &live, 1) || !ram(0x8d108, &task_lease, 1) ||
        !live || live != task_lease || live != w[1] || live != w[2] ||
        dispatch[0] != 0 || dispatch[1] != 0 || dispatch[2] != live ||
        w[4] != 2147516416 || w[6] != 0x801f0) {
        reject();
        return false;
    }
    *lease = live;
    return true;
}

static void stored(unsigned cpu, qemu_plugin_meminfo_t info, uint64_t va, void *data)
{
    struct site *s = data;
    if (cpu != s->cpu) {
        return;
    }
    bool entered = armed[cpu];
    armed[cpu] = false;
    struct qemu_plugin_hwaddr *hw = qemu_plugin_get_hwaddr(info, va);
    uint64_t expected = cpu ? SGIR : COUNT;
    if (!entered || !hw || va != expected || qemu_plugin_hwaddr_phys_addr(hw) != va ||
        qemu_plugin_hwaddr_is_io(hw) != (cpu == 1) ||
        !qemu_plugin_mem_is_store(info) || qemu_plugin_mem_is_big_endian(info) ||
        qemu_plugin_mem_size_shift(info) != (cpu ? 2 : 3)) {
        reject();
        return;
    }
    qemu_plugin_mem_value value = qemu_plugin_mem_get_value(info);
    uint64_t payload, lease = 0;
    if (cpu == 0) {
        if (value.type != QEMU_PLUGIN_MEM_VALUE_U64) {
            reject();
            return;
        }
        payload = value.data.u64;
        if (!final_row(payload, &lease)) {
            return;
        }
    } else {
        if (value.type != QEMU_PLUGIN_MEM_VALUE_U32 || value.data.u32 != 0x18001) {
            reject();
            return;
        }
        payload = value.data.u32;
    }
    if (atomic_load_explicit(&complete[cpu], memory_order_relaxed)) {
        reject();
        return;
    }
    atomic_store_explicit(&complete[cpu], true, memory_order_release);
    char line[160];
    g_snprintf(line, sizeof line, "MG2SP actor=%u peer=%u value=%" G_GUINT64_FORMAT
               " lease=%" G_GUINT64_FORMAT "\n", cpu, peer[cpu], payload, lease);
    qemu_plugin_outs(line);
}

static void translated(struct qemu_plugin_tb *tb, void *data)
{
    (void)data;
    for (size_t i = 0; i < qemu_plugin_tb_n_insns(tb); i++) {
        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb, i);
        for (unsigned j = 0; j < G_N_ELEMENTS(sites); j++) {
            struct site *s = &sites[j];
            if (qemu_plugin_insn_vaddr(insn) != s->pc) {
                continue;
            }
            uint32_t word;
            if (qemu_plugin_insn_size(insn) != 4 ||
                qemu_plugin_insn_data(insn, &word, sizeof word) != 4 ||
                GUINT32_FROM_LE(word) != s->word) {
                reject();
                continue;
            }
            qemu_plugin_register_vcpu_insn_exec_cb(insn, before, QEMU_PLUGIN_CB_NO_REGS, s);
            qemu_plugin_register_vcpu_mem_cb(insn, stored, QEMU_PLUGIN_CB_NO_REGS,
                                            QEMU_PLUGIN_MEM_W, s);
        }
    }
}

QEMU_PLUGIN_EXPORT int qemu_plugin_install(qemu_plugin_id_t id, const qemu_info_t *info,
                                           int argc, char **argv)
{
    (void)argv;
    if (argc || !info->system_emulation || strcmp(info->target_name, "aarch64") ||
        info->system.smp_vcpus != 4) {
        return -1;
    }
    qemu_plugin_register_vcpu_tb_trans_cb(id, translated, NULL);
    return 0;
}
