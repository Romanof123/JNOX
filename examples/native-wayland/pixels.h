#ifndef CLIENT_PIXELS_H
#define CLIENT_PIXELS_H
#include <stdint.h>
enum { WIDTH = 128, HEIGHT = 96, STRIDE = 4 * WIDTH, POOL_BYTES = STRIDE * HEIGHT };

static uint32_t paint(volatile uint32_t *pixels, int tiles, unsigned presses)
{
    static const uint32_t palette[2][3] = {
        {0x276575, 0x398657, 0x445a90}, {0x704567, 0xa25639, 0x73559b}
    };
    static const unsigned short motif[2][12] = {
        {0x0010,0x0030,0x0028,0x0028,0x0068,0x0448,0x0a44,0xf244,0x0182,0x0102,0x0101,0},
        {0x7e7e,0x4242,0x5a5a,0x5a5a,0x4242,0x7e7e,0,0x3c3c,0x2424,0x2424,0x3c3c,0}
    };
    /* PULSE / TILES, five columns per glyph, seven rows. */
    static const unsigned char titles[2][5][7] = {
        {{30,17,17,30,16,16,16},{17,17,17,17,17,17,14},{16,16,16,16,16,16,31},
         {15,16,16,14,1,1,30},{31,16,16,30,16,16,31}},
        {{31,4,4,4,4,4,4},{14,4,4,4,4,4,14},{16,16,16,16,16,16,31},
         {31,16,16,30,16,16,31},{15,16,16,14,1,1,30}}
    };
    unsigned origin = tiles ? 69 : 12;
    uint32_t ink = tiles ? 0xf1d5bc : 0xd2ece8;
    uint32_t body = palette[tiles][presses % 3], hash = 2166136261u;
    for (unsigned y = 0; y < HEIGHT; ++y) {
        for (unsigned x = 0; x < WIDTH; ++x) {
            uint32_t pixel = body;
            if (x < 2 || x >= WIDTH-2 || y < 2 || y >= HEIGHT-2) pixel = ink;
            else if (y < 20) {
                pixel = 0x233541;
                if (y >= 7 && y < 14 && x >= 8 && x < 38) {
                    unsigned cell = (x-8)/6, col = (x-8)%6;
                    if (col < 5 && (titles[tiles][cell][y-7] & (16u >> col))) pixel = 0xe0eee9;
                }
            } else if (y >= 33 && y < 69 && x >= origin && x < origin+48 &&
                       (motif[tiles][(y-33)/3] & (0x8000u >> ((x-origin)/3)))) {
                pixel = ink;
            }
            pixels[y * WIDTH + x] = pixel;
            hash = (hash ^ pixel) * 16777619u;
        }
    }
    return hash;
}
#endif
