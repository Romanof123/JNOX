#include <stdint.h>
_Static_assert(sizeof(uint8_t) == 1 && UINT8_MAX == 255, "uint8_t");
_Static_assert(sizeof(uint16_t) == 2 && UINT16_MAX == 65535, "uint16_t");
_Static_assert(sizeof(int64_t) == 8 && INT64_MIN == -9223372036854775807LL - 1 && INT64_MAX == 9223372036854775807LL, "int64_t");
_Static_assert(sizeof(intptr_t) == sizeof(void *) && sizeof(intptr_t) == 8 && INTPTR_MIN == INT64_MIN && INTPTR_MAX == INT64_MAX, "intptr_t");
int main(void) { return 0; }
