#include <stdlib.h>
int main(void) {
    void *m=malloc(0), *p=malloc(10);
    if (!p) return 99;
    void *r=realloc(p,0), *c=calloc(0,10);
    return 64 + (!m) + 2*(!r) + 4*(!c);
}
