/* One SDK ELF, three disjoint address spaces. The idle role observes returns. */
#include <stdint.h>

struct observation {
    uint64_t role;
    uint64_t generation;
    uint64_t first_tick;
    uint64_t chunks;
    uint64_t checksum;
    uint64_t exit_status;
};
volatile struct observation native_partition_result;
_Static_assert(sizeof(struct observation) == 48, "partition observation ABI");

static uint64_t acquire(const volatile uint64_t *word)
{
    uint64_t value;
    __asm__ volatile("ldar %0,[%1]" : "=r"(value) : "r"(word) : "memory");
    return value;
}

int main(int argc, char **argv)
{
    if (argc != 3 || !argv || !argv[1] || argv[1][1] || argv[3])
        return 90;
    char id = argv[1][0];
    unsigned role = id == 'A' ? 1 : id == 'B' ? 2 : id == 'I' ? 3 : 0;
    if (!role)
        return 90;
    native_partition_result.role = role;
    const volatile uint64_t *control = (const volatile uint64_t *)0x01200000UL;
    while (!acquire(control)) {
        uint64_t generation = acquire(control + 1);
        if (generation != native_partition_result.generation) {
            uint64_t tick;
            __asm__ volatile("isb\n mrs %0,cntpct_el0" : "=r"(tick) :: "memory");
            native_partition_result.first_tick = tick;
            native_partition_result.generation = generation;
        }
        if (role < 3) {
            ++native_partition_result.chunks;
            native_partition_result.checksum += role == 1 ? 3 : 5;
        }
    }
    unsigned status = 40 + role;
    native_partition_result.exit_status = status;
    return status;
}
