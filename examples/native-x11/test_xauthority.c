/* Run on the host against a private, single-record fixture from local_profile.py. */
#include <stdio.h>
#include <string.h>
#include "xauthority.h"

int main(int argc, char **argv)
{
    unsigned char record[256], cookie[MOSAIC_COOKIE_SIZE], altered[sizeof(record) + 1];
    if (argc != 3) return 1;
    FILE *file = fopen(argv[1], "rb");
    if (!file) return 2;
    size_t n = fread(record, 1, sizeof(record), file);
    int clean = !ferror(file) && feof(file);
    if (fclose(file) || !clean || n < MOSAIC_COOKIE_SIZE) return 3;
    size_t node = strlen(argv[2]);
    if (!local_auth_cookie(record, n, argv[2], node, cookie) ||
        memcmp(cookie, record + n - sizeof(cookie), sizeof(cookie))) return 4;
    for (size_t size = 0; size < n; ++size)
        if (local_auth_cookie(record, size, argv[2], node, cookie)) return 5;
    memcpy(altered, record, n);
    altered[n] = 0;
    if (local_auth_cookie(altered, n + 1, argv[2], node, cookie)) return 6;
    /* Change each selector separately, keeping all lengths well formed. */
    size_t positions[] = { 1, 4, 4 + node + 2, 4 + node + 2 + 1 + 2 };
    for (size_t i = 0; i < sizeof(positions) / sizeof(*positions); ++i) {
        memcpy(altered, record, n);
        altered[positions[i]] ^= 1;
        if (local_auth_cookie(altered, n, argv[2], node, cookie)) return 7;
    }
    memcpy(altered, record, n);
    altered[n - sizeof(cookie) - 1] = sizeof(cookie) - 1;
    if (local_auth_cookie(altered, n - 1, argv[2], node, cookie)) return 8;
    /* A nonmatching record can precede an exact match in a real Xauthority file. */
    unsigned char two[sizeof(record) * 2];
    memcpy(two, record, n);
    two[0] = two[1] = 255; /* FamilyWild */
    if (local_auth_cookie(two, n, argv[2], node, cookie)) return 9;
    memcpy(two + n, record, n);
    if (!local_auth_cookie(two, 2*n, argv[2], node, cookie) ||
        memcmp(cookie, record + n - sizeof(cookie), sizeof(cookie))) return 10;
    return 0;
}
