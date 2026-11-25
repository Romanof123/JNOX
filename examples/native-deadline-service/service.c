#include <jnox/deadline.h>
#include <stdint.h>

enum { cell_count = 16, batch_size = 64 };

struct service_receipt {
    uint32_t ordinal, cell, first, last, count, reserved;
    uint64_t squares;
};
_Static_assert(sizeof(struct service_receipt) == 32, "receipt must occupy 32 bytes");
_Static_assert(__builtin_offsetof(struct service_receipt, squares) == 24, "receipt sum offset");

volatile struct service_receipt service_receipts[cell_count];
volatile uint32_t service_receipt_count;
volatile int service_wait_ordinal[cell_count];
volatile uint32_t service_samples[batch_size];

static unsigned parse_cells(const char *text, unsigned cells[cell_count])
{
    unsigned count = 0;
    for (;;) {
        if (*text < '0' || *text > '9')
            return 0;
        unsigned cell = 0;
        do {
            cell = 10 * cell + (unsigned)(*text++ - '0');
            if (cell >= cell_count)
                return 0;
        } while (*text >= '0' && *text <= '9');
        if (count == cell_count || (count && cell <= cells[count - 1]))
            return 0;
        cells[count++] = cell;
        if (*text == '\0')
            return count;
        if (*text++ != ',')
            return 0;
    }
}

__attribute__((noinline))
uint64_t service_reduce(void)
{
    uint64_t total = 0;
    for (unsigned i = 0; i < batch_size; ++i) {
        uint64_t value = service_samples[i];
        total += value * value;
    }
    return total;
}

int main(int argc, char **argv)
{
    unsigned cells[cell_count];
    if (argc != 2 || !argv[1])
        return 101;
    unsigned activations = parse_cells(argv[1], cells);
    if (!activations)
        return 102;
    for (unsigned ordinal = 0; ordinal < activations; ++ordinal) {
        unsigned first = ordinal * batch_size + 1;
        for (unsigned i = 0; i < batch_size; ++i)
            service_samples[i] = first + i;
        uint64_t squares = service_reduce();
        volatile struct service_receipt *row = &service_receipts[ordinal];
        row->ordinal = ordinal;
        row->cell = cells[ordinal];
        row->first = first;
        row->last = first + batch_size - 1;
        row->count = batch_size;
        row->reserved = 0;
        row->squares = squares;
        service_receipt_count = ordinal + 1;
        if (ordinal + 1 < activations) {
            int next = jnox_deadline_service_wait_next();
            if (next != (int)(ordinal + 1))
                return 103;
            service_wait_ordinal[ordinal + 1] = next;
        }
    }
    return 41;
}
