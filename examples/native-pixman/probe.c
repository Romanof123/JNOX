#include <assert.h>
#include <errno.h>
#include <math.h>
#include <pixman.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

volatile uint64_t jnox_pixman_report[20];
static volatile double math_input = 2.0;
/* Picolibc supplies an inline AArch64 sqrt in its public header. Also
 * exercise the archive entry through an actual target function pointer. */
extern double linked_sqrt(double) __asm__("sqrt");
static double (*volatile math_sqrt)(double) = linked_sqrt;
extern int jnox_graphics_emit(const char *s, size_t size);
extern int graphics_fp_yield(uint64_t pattern);

__attribute__((constructor)) static void probe_init(void) { jnox_pixman_report[0] = 1; }
__attribute__((destructor)) static void probe_fini(void)
{
    jnox_pixman_report[1] = 1;
    const char line[] = "JNOX PIXMAN FINI p\n";
    if (jnox_pixman_report[2] == 1) (void)jnox_graphics_emit(line, sizeof(line)-1);
    if (jnox_pixman_report[2] == 2) {
        const char other[] = "JNOX PIXMAN FINI q\n";
        (void)jnox_graphics_emit(other, sizeof(other)-1);
    }
}

int main(int argc, char **argv)
{
    if (argc != 2 || !argv[1] || argv[1][1]) return 100;
    const char role = argv[1][0];
    if (role == 'a') { assert(role != 'a'); return 101; }
    if (role != 'p' && role != 'q') return 102;
    if (!jnox_pixman_report[0]) return 103;
    unsigned peer = role == 'q';
    jnox_pixman_report[2] = peer + 1;
    char begin[] = "JNOX PIXMAN FP BEGIN p\n";
    begin[sizeof(begin)-3] = role;
    if (jnox_graphics_emit(begin, sizeof(begin)-1)) return 112;
    for (unsigned i = 0; i < 32; ++i) {
        if (graphics_fp_yield(0x7654321000000000ULL + 2*i + peer)) return 104;
        jnox_pixman_report[3] = i + 1;
    }
    char end[] = "JNOX PIXMAN FP END p\n";
    end[sizeof(end)-3] = role;
    if (jnox_graphics_emit(end, sizeof(end)-1)) return 113;
    double root = math_sqrt(math_input);
    if (!(root > 1.41421 && root < 1.41422 && floor(root) == 1.0)) return 105;
    errno = 0;
    volatile size_t oversize = 1024u * 1024u;
    void *too_large = malloc(oversize);
    if (too_large || errno != ENOMEM) return 106;
    /* Each cycle allocates more than the entire arena in aggregate; a
     * bump-only allocator cannot finish. Preserve and check copied bytes. */
    for (unsigned i = 0; i < 24; ++i) {
        unsigned char *block = malloc(32768);
        if (!block) return 107;
        memset(block, (int)i, 32768);
        if (block[32767] != i) return 108;
        jnox_pixman_report[4] = (uintptr_t)block;
        free(block);
        jnox_pixman_report[5] = i + 1;
    }
    uint32_t source[4] = {0x80402010, 0, 0xff1267ab, 0x400f1001};
    uint32_t dest[4] = {0xff204060, 0xff123456, 0xff987654, 0xff346789};
    if (peer) source[0] = 0x80201040;
    pixman_image_t *s = pixman_image_create_bits(PIXMAN_a8r8g8b8, 2, 2, source, 8);
    pixman_image_t *d = pixman_image_create_bits(PIXMAN_a8r8g8b8, 2, 2, dest, 8);
    if (!s || !d) return 109;
    pixman_image_composite32(PIXMAN_OP_OVER, s, NULL, d, 0, 0, 0, 0, 0, 0, 2, 2);
    for (unsigned i = 0; i < 4; ++i) jnox_pixman_report[6+i] = dest[i];
    if (!pixman_image_unref(s) || !pixman_image_unref(d)) return 110;
    char line[96];
    int n = snprintf(line, sizeof(line), "JNOX PIXMAN %c %08lx %08lx %08lx %08lx\n", role,
        (unsigned long)dest[0], (unsigned long)dest[1], (unsigned long)dest[2],
        (unsigned long)dest[3]);
    if (n <= 0 || (size_t)n >= sizeof(line) || jnox_graphics_emit(line, (size_t)n)) return 111;
    return peer ? 72 : 71;
}
