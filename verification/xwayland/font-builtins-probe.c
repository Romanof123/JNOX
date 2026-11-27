/* Read the actual upstream built-in PCF through libXfont2's gzip reader. */
#include <stdint.h>
#include <string.h>
#include "builtin.h"

static unsigned char pcf[19584];
volatile uint8_t jnox_font_probe_rows[13];

static unsigned le16(const unsigned char *p)
{
    return p[0] | ((unsigned)p[1] << 8);
}

static uint32_t le32(const unsigned char *p)
{
    return le16(p) | ((uint32_t)le16(p + 2) << 16);
}

static unsigned table(unsigned kind)
{
    for (unsigned i = 0; i < 9; ++i) {
        const unsigned char *t = pcf + 8 + 16 * i;
        if (le32(t) == kind && le32(t + 4) == 2) {
            unsigned size = le32(t + 8), offset = le32(t + 12);
            if (offset <= sizeof pcf && size <= sizeof pcf - offset)
                return offset;
        }
    }
    return 0;
}

int main(void)
{
    static const unsigned char expected[] =
        {0, 0, 4, 10, 17, 17, 17, 31, 17, 17, 17, 0, 0};
    FontFilePtr f = BuiltinFileOpen("6x13.builtin");
    if (!f) return 1;
    int n = FontFileRead(f, (char *)pcf, sizeof pcf);
    int eof = FontFileGetc(f);
    BuiltinFileClose(f, 0);
    if (n != sizeof pcf || eof >= 0 || memcmp(pcf, "\001fcp", 4) || le32(pcf+4) != 9)
        return 2;
    unsigned encoding = table(32), bitmap = table(8);
    if (encoding != 15628 || bitmap != 1992 || le32(pcf+bitmap+4) != 223)
        return 3;
    if (le16(pcf+encoding+4) != 0 || le16(pcf+encoding+6) != 255 ||
        le16(pcf+encoding+8) != 0 || le16(pcf+encoding+10) != 0)
        return 4;
    unsigned glyph = le16(pcf+encoding+14+2*'A');
    if (glyph != 65) return 5;
    unsigned offset = le32(pcf+bitmap+8+4*glyph);
    if (offset > sizeof pcf - (bitmap+8+4*223+16) - 52) return 6;
    const unsigned char *bits = pcf+bitmap+8+4*223+16+offset;
    for (unsigned i = 0; i < 13; ++i) {
        jnox_font_probe_rows[i] = bits[4*i];
        if (bits[4*i] != expected[i]) return 7;
    }
#ifdef JNOX_FONT_UART
    extern int jnox_graphics_emit(const char *, size_t);
    static const char hex[] = "0123456789abcdef";
    char report[] = "FONT ROWS 00000000000000000000000000\n";
    for (unsigned i = 0; i < 13; ++i) {
        unsigned value = jnox_font_probe_rows[i];
        report[10 + 2*i] = hex[value >> 4];
        report[11 + 2*i] = hex[value & 15];
    }
    if (jnox_graphics_emit(report, sizeof(report)-1)) return 8;
#endif
    return 0;
}
