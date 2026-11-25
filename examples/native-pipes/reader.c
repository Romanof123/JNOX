#include <unistd.h>
#include <fcntl.h>
#include "record.h"

volatile unsigned char pipe_received[2 * PER_WRITER][RECORD_BYTES];
volatile unsigned short pipe_read_sizes[128];
volatile uint32_t pipe_reads, pipe_records, pipe_eof;
int main(void)
{
    unsigned char chunk[137], record[RECORD_BYTES];
    char gate;
    unsigned count[2] = {0, 0}, used = 0;
    if (read(3, &gate, 1) != 1 || gate != 'o') return 91;
    int input = open(FIFO_NAME, O_RDONLY);
    if (input < 0 || read(3, &gate, 1) != 1 || gate != 'r' || close(3)) return 92;
    for (;;) {
        ssize_t n = read(input, chunk, sizeof chunk);
        if (n < 0 || pipe_reads >= 128) return 93;
        pipe_read_sizes[pipe_reads++] = (unsigned short)n;
        if (n == 0) { pipe_eof = 1; break; }
        for (ssize_t i = 0; i < n; ++i) {
            record[used++] = chunk[i];
            if (used < RECORD_BYTES) continue;
            unsigned who = record[0], seq = record[1];
            if (who > 1 || seq >= PER_WRITER || seq != count[who]++) return 94;
            for (unsigned j = 2; j < 508; ++j)
                if (record[j] != sample(who, seq, j)) return 95;
            uint32_t crc = record_crc(record);
            for (unsigned j = 0; j < 4; ++j)
                if (record[508+j] != (unsigned char)(crc >> (8*j))) return 96;
            for (unsigned j = 0; j < RECORD_BYTES; ++j)
                pipe_received[pipe_records][j] = record[j];
            ++pipe_records;
            used = 0;
        }
    }
    if (used || count[0] != PER_WRITER || count[1] != PER_WRITER || close(input)) return 97;
    return 63;
}
