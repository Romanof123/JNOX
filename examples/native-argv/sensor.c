#include <stdint.h>
#include <sched.h>

volatile uint32_t sensor_argc, sensor_lengths[16], sensor_count, sensor_min, sensor_max, sensor_yields;
volatile uint64_t sensor_sum;
volatile unsigned char sensor_label[1024];
static volatile unsigned char ramp[256];

static int equals(const char *input, const char *literal)
{
    while (*input && *input == *literal) {
        ++input;
        ++literal;
    }
    return *input == *literal;
}

static int endpoint(const char *text, unsigned *value)
{
    if (!*text)
        return 0;
    unsigned n = 0;
    for (; *text; ++text) {
        if (*text < '0' || *text > '9')
            return 0;
        n = n * 10 + (unsigned)(*text - '0');
        if (n > 256)
            return 0;
    }
    *value = n;
    return 1;
}

int main(int argc, char **argv)
{
    if (argc < 1 || argc > 16 || !argv || argv[argc] || argv[argc + 1])
        return 101;
    sensor_argc = (unsigned)argc;
    for (int i = 0; i < argc; ++i) {
        if (!argv[i])
            return 102;
        unsigned n = 0;
        while (n < 1024 && argv[i][n])
            ++n;
        if (n == 1024)
            return 103;
        sensor_lengths[i] = n;
    }
    if (!equals(argv[0], "/bin/app"))
        return 104;
    if (argc == 1)
        return 0;
    if (argc != 5)
        return 105;
    int sum = equals(argv[1], "sum");
    unsigned low, high;
    if ((!sum && !equals(argv[1], "minmax")) ||
        !endpoint(argv[2], &low) || !endpoint(argv[3], &high) || low >= high)
        return 106;
    const volatile unsigned char *label = (const volatile unsigned char *)argv[4];
    unsigned length = sensor_lengths[4];
    for (unsigned j = 0; j <= length; ++j)
        sensor_label[j] = label[j];
    for (unsigned i = 0; i < 256; ++i)
        ramp[i] = (unsigned char)i;
    sensor_min = 255;
    for (unsigned i = low; i < high; ++i) {
        unsigned value = ramp[i];
        if (sum)
            sensor_sum += value;
        else {
            if (value < sensor_min)
                sensor_min = value;
            if (value > sensor_max)
                sensor_max = value;
        }
        if (++sensor_count % 4 == 0) {
            volatile unsigned saved = low + sensor_count;
            if (sched_yield() || saved != i + 1)
                return 107;
            for (unsigned j = 0; j <= length; ++j)
                if (label[j] != sensor_label[j])
                    return 108;
            ++sensor_yields;
        }
    }
    if (sensor_count != high - low)
        return 109;
    if (sum)
        return sensor_sum == (uint64_t)(high - low) * (low + high - 1) / 2 ? 17 : 110;
    return sensor_min == low && sensor_max == high - 1 ? 29 : 111;
}
