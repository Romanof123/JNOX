#include <config.h>

VM_IMAGE(linux_guest, "configs/domains/linux.bin")
VM_IMAGE(jnox_guest, "configs/domains/jnox.bin")

#ifdef JNOX_GATEWAY
/* Both guests map the same page and poll it. Bao reads these IPC descriptors. */
static struct ipc gateway_ipcs[] = {
    {.base = 0x0b000000, .size = 0x1000, .shmem_id = 0, .interrupt_num = 0},
};
#endif

struct config config = {
#ifdef JNOX_GATEWAY
    .shmemlist_size = 1,
    .shmemlist = (struct shmem[]) {
        {.size = 0x1000, .place_phys = true, .phys = 0x5e000000},
    },
#endif
    .vmlist_size = 2,
    .vmlist = (struct vm_config[]) {
        {
            .image = VM_IMAGE_BUILTIN(linux_guest, 0x50000000),
            .entry = 0x50000000,
            .cpu_affinity = 1,
            .platform = {
                .cpu_num = 1,
#ifdef JNOX_GATEWAY
                .ipc_num = 1,
                .ipcs = gateway_ipcs,
#endif
                .region_num = 1,
                .regions = (struct vm_mem_region[]) {
                    {.base = 0x50000000, .size = 0x0c000000, .place_phys = true, .phys = 0x50000000},
                },
                .dev_num = 3,
                .devs = (struct vm_dev_region[]) {
                    {.pa = 0x09000000, .va = 0x09000000, .size = 0x1000,
                     .interrupt_num = 1, .interrupts = (irqid_t[]) {33}},
                    /* The NIC at 0x0a003e00 shares this stage-2 page with seven empty slots. */
                    {.pa = 0x0a003000, .va = 0x0a003000, .size = 0x1000,
                     .interrupt_num = 1, .interrupts = (irqid_t[]) {79}},
                    {.va = INVALID_VA, .interrupt_num = 1, .interrupts = (irqid_t[]) {27}},
                },
                .arch.gic = {.gicd_addr = 0x08000000, .gicc_addr = 0x08010000},
            },
        },
        {
            .image = VM_IMAGE_BUILTIN(jnox_guest, 0x40200000),
            .entry = 0x40200000,
            .cpu_affinity = 2,
            .platform = {
                .cpu_num = 1,
#ifdef JNOX_GATEWAY
                .ipc_num = 1,
                .ipcs = gateway_ipcs,
#endif
                .region_num = 1,
                .regions = (struct vm_mem_region[]) {
                    {.base = 0x40000000, .size = 0x08000000, .place_phys = true, .phys = 0x48000000},
                },
                .dev_num = 1,
                .devs = (struct vm_dev_region[]) {
                    {.va = INVALID_VA, .interrupt_num = 1, .interrupts = (irqid_t[]) {27}},
                },
                .arch.gic = {.gicd_addr = 0x08000000, .gicc_addr = 0x08010000},
            },
        },
    },
};
