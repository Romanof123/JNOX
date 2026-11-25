#include <stdint.h>

struct observation { uint64_t counter, completed, witness; };
_Static_assert(sizeof(struct observation) == 24, "observation row size");
volatile struct observation records[8192];
volatile uint32_t record_count, record_overflow;
volatile uint64_t private_blocks, private_result, observed_frequency, first_counter, final_counter;
static volatile unsigned char ramp[256];

static inline uint64_t counter(void)
{
    uint64_t t;
    __asm__ volatile("isb\nmrs %0, cntpct_el0" : "=r"(t) :: "memory");
    return t;
}

static void append(uint64_t t)
{
    uint32_t i = record_count;
    if (i == 8192) {
        record_overflow = 1;
        return;
    }
    records[i].counter = t;
    records[i].completed = private_blocks;
    records[i].witness = private_result;
    record_count = i + 1;
}

__attribute__((noinline))
static uint64_t block_energy(void)
{
    uint64_t energy = 0;
    for (unsigned i = 0; i < 256; ++i) {
        unsigned x = ramp[i];
        energy += x * x;
    }
    return energy;
}

int main(void)
{
    uint64_t start = counter(), frequency;
    __asm__ volatile("mrs %0, cntfrq_el0" : "=r"(frequency));
    first_counter = start;
    observed_frequency = frequency;
    append(start);
    if (!frequency || frequency % 1000)
        return 101;
    uint64_t interval = frequency / 5000 + (frequency % 5000 != 0);
    uint64_t duration = frequency / 1000 * 900;
    uint64_t last = start;
    for (unsigned i = 0; i < 256; ++i)
        ramp[i] = (unsigned char)i;
    for (;;) {
        uint64_t energy = block_energy();
        if (energy > UINT64_MAX - private_result || private_blocks == UINT64_MAX)
            return 102;
        private_result += energy;
        ++private_blocks;
        uint64_t now = counter();
        if (now < last)
            return 103;
        if (now - last >= interval) {
            append(now);
            last = now;
        }
        if (now - start >= duration) {
            final_counter = now;
            if (private_blocks > UINT64_MAX / 5559680 ||
                private_result != private_blocks * 5559680)
                return 104;
            return 41;
        }
    }
}
