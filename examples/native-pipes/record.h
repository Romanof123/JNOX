#ifndef NATIVE_PIPE_RECORD_H
#define NATIVE_PIPE_RECORD_H
#include <stdint.h>
enum { RECORD_BYTES = 512, PER_WRITER = 8 };
#define FIFO_NAME "/samples"
static unsigned char sample(unsigned origin, unsigned sequence, unsigned index)
{
    return (unsigned char)(origin * 97u + sequence * 31u + index * 17u + (index >> 3));
}
static uint32_t record_crc(const unsigned char *data)
{
    uint32_t crc = 0xffffffffu;
    for (unsigned i = 0; i < RECORD_BYTES - 4; ++i) {
        crc ^= data[i];
        for (unsigned j = 0; j < 8; ++j)
            crc = (crc >> 1) ^ ((0u - (crc & 1u)) & 0xedb88320u);
    }
    return ~crc;
}
#endif
