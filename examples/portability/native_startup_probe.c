/* CHG-087 native bootstrap profile: /bin/native_startup_probe 8 13 */
#include <errno.h>
#include <unistd.h>

static volatile unsigned products[2];
static volatile unsigned bias = 8;
static const unsigned weights[] = {2, 1};

static int same_string(const char *value, const char *expected)
{
    if (!value)
        return 0;
    while (*expected && *value == *expected) {
        ++value;
        ++expected;
    }
    return *value == *expected;
}

/* Read through a volatile view so the arithmetic consumes linked RO bytes. */
__attribute__((noinline))
static void multiply(unsigned left, unsigned right,
                     const volatile unsigned *coefficients)
{
    products[0] = left * coefficients[0];
    products[1] = right * coefficients[1];
}

int main(int argc, char **argv)
{
    /* The native launcher appends an empty environment after argv's NULL. */
    if (argc != 3 || !argv || argv[3] || argv[4])
        return 90;
    if (!same_string(argv[0], "/bin/native_startup_probe") ||
        !same_string(argv[1], "8") || !same_string(argv[2], "13"))
        return 91;
    if (products[0] != 0 || products[1] != 0 || bias != 8)
        return 92;

    const volatile char *first = argv[1];
    const volatile char *second = argv[2];
    unsigned left = (unsigned)(first[0] - '0');
    unsigned right = 10u * (unsigned)(second[0] - '0') + (unsigned)(second[1] - '0');
    multiply(left, right, weights);
    unsigned result = products[0] + products[1] + bias;
    if (products[0] != 16 || products[1] != 13 || result != 37)
        return 93;

    errno = 0;
    if (write(1, "37\n", 3) != -1)
        return 94;
    if (errno != ENOSYS)
        return 95;
    return (int)result;
}
