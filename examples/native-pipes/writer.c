#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#include <sys/stat.h>
#include "record.h"

volatile uint32_t c_writer_crc[PER_WRITER], c_write_count;
volatile int c_full_rc, c_full_errno;

static int emit(int fd, unsigned seq)
{
    unsigned char data[RECORD_BYTES];
    for (unsigned i = 0; i < RECORD_BYTES - 4; ++i)
        data[i] = sample(0, seq, i);
    data[0] = 0;
    data[1] = (unsigned char)seq;
    uint32_t crc = record_crc(data);
    for (unsigned j = 0; j < 4; ++j)
        data[508+j] = (unsigned char)(crc >> (8*j));
    ssize_t rc = write(fd, data, sizeof data);
    if (rc == RECORD_BYTES) {
        c_writer_crc[seq] = crc;
        ++c_write_count;
    }
    return (int)rc;
}

int main(void)
{
    char ack;
    if (mkfifo(FIFO_NAME, 0600)) return 90;
    if (write(4, "o", 1) != 1) return 91;
    int out = open(FIFO_NAME, O_WRONLY);
    if (out < 0 || fcntl(out, F_SETFL, O_NONBLOCK)) return 91;
    for (unsigned i = 0; i < 4; ++i)
        if (emit(out, i) != RECORD_BYTES) return 92;
    c_full_rc = emit(out, 4);
    c_full_errno = errno;
    if (c_full_rc != -1 || c_full_errno != EAGAIN || fcntl(out, F_SETFL, 0)) return 93;
    // Reader has rendezvoused but waits for a second gate before draining.
    if (write(3, "w", 1) != 1 || write(4, "r", 1) != 1 ||
        read(5, &ack, 1) != 1 || ack != 'a') return 94;
    // Both writers hold a description before the C writer can close.
    for (unsigned i = 4; i < PER_WRITER; ++i)
        if (emit(out, i) != RECORD_BYTES) return 95;
    if (unlink(FIFO_NAME) || close(out) || close(3) || close(4) || close(5)) return 96;
    return 61;
}
