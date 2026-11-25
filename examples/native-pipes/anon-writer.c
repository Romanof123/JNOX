#include <unistd.h>
volatile unsigned anon_writer_steps;
int main(void)
{
    char ack;
    int held = dup(1);
    if (held < 0 || write(1, "rain", 4) != 4 || close(1)) return 90;
    anon_writer_steps = 1;
    if (read(4, &ack, 1) != 1 || ack != 'a' || close(4)) return 91;
    if (write(held, "fall", 4) != 4 || close(held)) return 92;
    anon_writer_steps = 2;
    return 64;
}
