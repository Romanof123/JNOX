#include <stdint.h>

struct report {
    uint64_t phase, role, count, scalar, vector, signature, lane0, lane1;
    uint64_t initial_tp, live_tp, initial_ro, final_ro, fpcr, fpsr, tls_word, restored_tp;
};
_Static_assert(sizeof(struct report) == 128, "assembly report ABI");
volatile struct report smp_abi_result;
_Alignas(32) volatile uint64_t smp_abi_tls[2];

extern void smp_abi_work(uint64_t role, volatile struct report *out,
                         volatile uint64_t *tls, uint64_t signature);

int main(void)
{
    const uint64_t n = 33554432ULL;
    const uint64_t signature = ROLE ? 0xaaaabbbbccccddddULL : 0x1111222233334444ULL;
    const uint64_t answer = n * (n + ROLE);
    volatile struct report *r = &smp_abi_result;
    volatile uint64_t *cell = &smp_abi_tls[ROLE];
    smp_abi_work(ROLE, r, cell, signature);
    if (r->phase != 2 || r->role != ROLE || r->count != n ||
        r->scalar != answer || r->vector != answer || r->signature != signature ||
        r->lane0 != (ROLE ? 0x201u : 0x101u) || r->lane1 != r->lane0 + 1 ||
        r->live_tp != (__UINTPTR_TYPE__)cell || r->tls_word != signature || *cell != signature ||
        r->restored_tp != r->initial_tp || r->initial_ro || r->final_ro ||
        r->fpcr != ((uint64_t)(ROLE + 1) << 22) ||
        r->fpsr != (ROLE ? 0u : 1u << 27))
        return 81 + ROLE;
    r->phase = 3;
    return ROLE ? 77 : 12;
}
