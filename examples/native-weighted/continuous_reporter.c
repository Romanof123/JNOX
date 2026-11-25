#include <stdint.h>

/* Measure the total squared amplitude of 128 Mi signed eight-bit samples. */
static volatile signed char samples[256];
volatile uint32_t private_progress;
volatile uint64_t private_result;

__attribute__((noinline))
void measure_energy(void)
{
    for (int i = 0; i < 256; ++i)
        samples[i] = (signed char)(i - 128);
    uint64_t energy = 0;
    for (unsigned block = 0; block < 524288; ++block) {
        for (unsigned i = 0; i < 256; ++i) {
            int sample = samples[i];
            energy += (unsigned)(sample * sample);
        }
        private_result = energy;
        private_progress = (block + 1) * 256;
    }
}

int main(void)
{
    measure_energy();
    uint64_t energy = private_result;
    return energy == 733030121472ULL ? (int)(energy % 251) : 103;
}
