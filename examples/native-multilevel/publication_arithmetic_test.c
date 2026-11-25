#include <assert.h>
#include "publication.h"

/* Host arithmetic control, built independently from the guest reporter. */
int main(void)
{
    char *args[] = {"report", "--publication-deadline-ms=40", 0};
    uint64_t value;
    assert(publication_setting(1, args, &value) && value == 0);
    assert(publication_setting(2, args, &value) && value == 40000000);
    assert(!publication_setting(3, args, &value));
    args[1] = "--publication-deadline-ms=18446744073709";
    assert(publication_setting(2, args, &value) && value == UINT64_C(18446744073709000000));
    char *invalid[] = {"", "--unknown", "--publication-deadline-ms=", "--publication-deadline-ms=0",
                       "--publication-deadline-ms=-1", "--publication-deadline-ms=+1",
                       "--publication-deadline-ms= 1", "--publication-deadline-ms=1x",
                       "--publication-deadline-ms=18446744073710"};
    for (unsigned i = 0; i < sizeof invalid / sizeof invalid[0]; ++i) {
        args[1] = invalid[i];
        assert(!publication_setting(2, args, &value));
    }
    assert(publication_target(UINT64_MAX - 40000000, &value) && value == UINT64_MAX);
    assert(!publication_target(UINT64_MAX - 39999999, &value));
    assert(publication_on_time(100, 40000100, 40000000));
    assert(!publication_on_time(100, 40000101, 40000000));
    assert(!publication_on_time(100, 100, 40000000));
    assert(!publication_on_time(100, 99, 40000000));
    return 0;
}
