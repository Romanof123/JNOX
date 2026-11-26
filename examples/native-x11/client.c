/* Freestanding X11 core client for the JNOX native socket SDK. */
#include <arpa/inet.h>
#include <errno.h>
#include <jnox/display.h>
#include <sys/socket.h>

typedef unsigned char byte;
enum { WIDTH = 240, HEIGHT = 160, BROKER = 1, SOCKET_CHUNK = 64, STRIP_ROWS = 32 };
enum { LEFT = (JNOX_DISPLAY_WIDTH - WIDTH) / 2, TOP = (JNOX_DISPLAY_HEIGHT - HEIGHT) / 2 };
_Static_assert(HEIGHT % STRIP_ROWS == 0, "GetImage strips must cover the window");
enum { CREATE_WINDOW = 1, DESTROY_WINDOW = 4, MAP_WINDOW = 8, CHANGE_PROPERTY = 18,
       CREATE_GC = 55, CHANGE_GC = 56, POLY_FILL_RECTANGLE = 70, GET_IMAGE = 73 };
enum { BUTTON_PRESS = 4, EXPOSE = 12, DESTROY_NOTIFY = 17 };

struct display {
    int fd;
    uint32_t root, window, gc, visual;
    uint32_t red, green, blue;
    unsigned depth;
    unsigned scanout, sequence, bytes_per_pixel, stride, msb_first;
    unsigned strips_left, reply_sequence, redraw_pending;
};

static unsigned u16(const byte *p)
{
    return p[0] | (unsigned)p[1] << 8;
}

static uint32_t u32(const byte *p)
{
    return u16(p) | (uint32_t)u16(p + 2) << 16;
}

static void put16(byte *p, unsigned v)
{
    p[0] = (byte)v;
    p[1] = (byte)(v >> 8);
}

static void put32(byte *p, uint32_t v)
{
    put16(p, v);
    put16(p + 2, v >> 16);
}

static unsigned port_number(const char *text)
{
    unsigned port = 0;
    for (; *text; ++text) {
        if (*text < '0' || *text > '9') return 0;
        port = 10 * port + (unsigned)(*text - '0');
        if (port > 65535) return 0;
    }
    return port >= 1024 ? port : 0;
}

static int nibble(char ch)
{
    if (ch >= '0' && ch <= '9') return ch - '0';
    if (ch >= 'a' && ch <= 'f') return ch - 'a' + 10;
    if (ch >= 'A' && ch <= 'F') return ch - 'A' + 10;
    return -1;
}

static int send_all(int fd, const byte *data, unsigned size)
{
    while (size) {
        unsigned take = size < SOCKET_CHUNK ? size : SOCKET_CHUNK;
        ssize_t n = send(fd, data, take, 0);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0 || (unsigned)n > take) return 0;
        data += n;
        size -= (unsigned)n;
    }
    return 1;
}

static int read_all(int fd, byte *data, unsigned size)
{
    while (size) {
        unsigned take = size < SOCKET_CHUNK ? size : SOCKET_CHUNK;
        ssize_t n = recv(fd, data, take, 0);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0 || (unsigned)n > take) return 0;
        data += n;
        size -= (unsigned)n;
    }
    return 1;
}

/* X uses the low 16 bits of the request sequence in replies. A rectangle
 * sends two requests together. Connection setup has no sequence number. */
static int requests(struct display *d, const byte *data, unsigned size, unsigned count)
{
    if (!send_all(d->fd, data, size)) return 0;
    d->sequence = (d->sequence + count) & 65535;
    return 1;
}

static int send_setup(int fd, const char *cookie)
{
    static const char name[] = "MIT-MAGIC-COOKIE-1";
    byte request[48] = {'l', 0, 11, 0, 0, 0, sizeof(name) - 1, 0, 16};
    for (unsigned i = 0; i < sizeof(name) - 1; ++i) request[12 + i] = name[i];
    for (unsigned i = 0; i < 16; ++i) {
        int hi = nibble(*cookie++);
        if (hi < 0) return 0;
        int lo = nibble(*cookie++);
        if (lo < 0) return 0;
        request[32 + i] = (byte)(hi * 16 + lo);
    }
    return !*cookie && send_all(fd, request, sizeof(request));
}

struct cursor {
    const byte *data;
    unsigned remaining;
};

static const byte *take(struct cursor *c, unsigned length)
{
    if (length > c->remaining) return 0;
    const byte *p = c->data;
    c->data += length;
    c->remaining -= length;
    return p;
}

static int valid_channel(uint32_t mask)
{
    if (!mask) return 0;
    mask >>= __builtin_ctz(mask);
    return !(mask & (mask + 1));
}

static int select_visual(struct display *d, const byte *v)
{
    d->red = u32(v + 8);
    d->green = u32(v + 12);
    d->blue = u32(v + 16);
    uint32_t rgb = d->red | d->green | d->blue;
    return v[4] == 4 /* TrueColor */ && valid_channel(d->red) &&
           valid_channel(d->green) && valid_channel(d->blue) &&
           !(d->red & d->green) && !(d->red & d->blue) && !(d->blue & d->green) &&
           (d->depth == 32 || !(rgb >> d->depth));
}

static int read_setup(struct display *d)
{
    /* Keep the variable-length setup off the user stack. No dynamic allocation. */
    static byte payload[65536];
    byte header[8];
    if (!read_all(d->fd, header, sizeof(header)) || header[0] != 1 ||
        u16(header + 2) != 11 || u16(header + 4) != 0) return 0;
    unsigned size = u16(header + 6) * 4;
    if (size > sizeof(payload) || !read_all(d->fd, payload, size)) return 0;
    struct cursor c = {payload, size};
    const byte *setup = take(&c, 32);
    if (!setup || !setup[20] || !setup[21] || u16(setup + 18) < 10) return 0;
    uint32_t base = u32(setup + 4), mask = u32(setup + 8);
    uint32_t first_bit = mask & -mask;
    uint32_t rest = mask & ~first_bit;
    if ((base & mask) || !rest) return 0;
    d->window = base | first_bit;
    d->gc = base | (rest & -rest);
    if (!take(&c, (u16(setup + 16) + 3u) & ~3u)) return 0; /* Padded vendor string. */
    const byte *formats = take(&c, 8u * setup[21]);
    if (!formats) return 0;
    for (unsigned s = 0; s < setup[20]; ++s) {
        const byte *screen = take(&c, 40);
        if (!screen) return 0;
        if (s == 0) {
            d->root = u32(screen);
            d->visual = u32(screen + 32);
            d->depth = screen[38];
            if (!d->root || !d->visual || d->depth == 0 || d->depth > 32) return 0;
            unsigned f = 0;
            while (f < setup[21] && formats[8*f] != d->depth) ++f;
            if (f == setup[21]) return 0;
            const byte *fmt = formats + 8*f;
            if (fmt[1] < d->depth || fmt[1] > 32 ||
                (fmt[2] != 8 && fmt[2] != 16 && fmt[2] != 32)) return 0;
            if (d->scanout) {
                if (setup[22] > 1 || fmt[1] < 8 || fmt[1] % 8) return 0;
                d->msb_first = setup[22];
                d->bytes_per_pixel = fmt[1] / 8;
                d->stride = ((WIDTH * fmt[1] + fmt[2] - 1) / fmt[2]) * (fmt[2] / 8);
                if (d->stride > WIDTH * 4) return 0;
            }
        }
        for (unsigned i = 0; i < screen[39]; ++i) {
            const byte *depth = take(&c, 8);
            if (!depth) return 0;
            unsigned count = u16(depth + 2);
            const byte *visuals = take(&c, 24u * count);
            if (!visuals) return 0;
            if (s == 0 && depth[0] == d->depth) {
                for (unsigned j = 0; j < count; ++j) {
                    const byte *v = visuals + 24*j;
                    if (u32(v) == d->visual && !select_visual(d, v)) return 0;
                }
            }
        }
    }
    return !c.remaining && d->red;
}

static uint32_t channel(unsigned value, uint32_t mask)
{
    unsigned shift = (unsigned)__builtin_ctz(mask);
    uint64_t scaled = ((uint64_t)value * (mask >> shift) + 127) / 255;
    return (uint32_t)scaled << shift;
}

static uint32_t pixel(const struct display *d, unsigned rgb)
{
    return channel(rgb >> 16, d->red) | channel((rgb >> 8) & 255, d->green) |
           channel(rgb & 255, d->blue);
}

static int window_request(struct display *d, byte opcode)
{
    byte req[8] = {opcode, 0, 2};
    put32(req + 4, d->window);
    return requests(d, req, sizeof(req), 1);
}

static int open_window(struct display *d)
{
    byte win[40] = {CREATE_WINDOW, 0, 10};
    put32(win + 4, d->window);
    put32(win + 8, d->root);
    put16(win + 12, 32);
    put16(win + 14, 32);
    put16(win + 16, WIDTH);
    put16(win + 18, HEIGHT);
    put16(win + 22, 1);  /* InputOutput, inherit the root depth and visual. */
    put32(win + 28, (1u << 1) | (1u << 11));  /* background-pixel, event-mask */
    put32(win + 32, pixel(d, 0x182840));
    put32(win + 36, (1u << 2) | (1u << 15) | (1u << 17)); /* ButtonPress, Exposure, StructureNotify */
    static const char title[] = "JNOX X11 Mosaic";
    byte name[24 + ((sizeof(title) - 1 + 3) & ~3u)] = {CHANGE_PROPERTY, 0};
    put16(name + 2, sizeof(name) / 4);
    put32(name + 4, d->window);
    put32(name + 8, 39);  /* predefined atom WM_NAME */
    put32(name + 12, 31); /* predefined atom STRING */
    name[16] = 8;
    put32(name + 20, sizeof(title) - 1);
    for (unsigned i = 0; i < sizeof(title) - 1; ++i) name[24 + i] = title[i];
    byte gc[16] = {CREATE_GC, 0, 4};
    put32(gc + 4, d->gc);
    put32(gc + 8, d->window);
    return requests(d, win, sizeof(win), 1) && requests(d, name, sizeof(name), 1) &&
           requests(d, gc, sizeof(gc), 1) && window_request(d, MAP_WINDOW);
}

static int rectangle(struct display *d, unsigned rgb, unsigned x, unsigned y,
                     unsigned width, unsigned height)
{
    /* Two complete, unbuffered X requests. The GC foreground applies to this rectangle. */
    byte req[36] = {CHANGE_GC, 0, 4};
    put32(req + 4, d->gc);
    put32(req + 8, 1u << 2);
    put32(req + 12, pixel(d, rgb));
    req[16] = POLY_FILL_RECTANGLE;
    put16(req + 18, 5);
    put32(req + 20, d->window);
    put32(req + 24, d->gc);
    put16(req + 28, x);
    put16(req + 30, y);
    put16(req + 32, width);
    put16(req + 34, height);
    return requests(d, req, sizeof(req), 2);
}

/* Queue all five reads before another input handler can change the drawable.
 * Each reply is consumed one wire row at a time by the regular event loop. */
static int request_frame(struct display *d)
{
    if (!d->scanout) return 1;
    if (d->strips_left) {
        d->redraw_pending = 1;
        return 1;
    }
    byte req[20] = {GET_IMAGE, 2, 5}; /* ZPixmap, full plane mask */
    put32(req + 4, d->window);
    put16(req + 12, WIDTH);
    put16(req + 14, STRIP_ROWS);
    put32(req + 16, 0xffffffffu);
    d->reply_sequence = (d->sequence + 1) & 65535;
    d->strips_left = HEIGHT / STRIP_ROWS;
    d->redraw_pending = 0;
    for (unsigned y = 0; y < HEIGHT; y += STRIP_ROWS) {
        put16(req + 10, y);
        if (!requests(d, req, sizeof(req), 1)) return 0;
    }
    return 1;
}

static unsigned unpack_channel(uint32_t raw, uint32_t mask)
{
    unsigned shift = (unsigned)__builtin_ctz(mask);
    unsigned maximum = mask >> shift;
    unsigned component = (raw & mask) >> shift;
    return (unsigned)(((uint64_t)component * 255 + maximum / 2) / maximum);
}

static void decode_row(const struct display *d, const byte *wire, uint32_t *pixels)
{
    if (d->bytes_per_pixel == 4 && !d->msb_first &&
        d->red == 0xff0000 && d->green == 0xff00 && d->blue == 0xff) {
        for (unsigned x = 0; x < WIDTH; ++x) pixels[x] = u32(wire + 4*x) & 0xffffff;
        return;
    }
    for (unsigned x = 0; x < WIDTH; ++x, wire += d->bytes_per_pixel) {
        uint32_t raw = 0;
        for (unsigned j = 0; j < d->bytes_per_pixel; ++j) {
            unsigned at = d->msb_first ? j : d->bytes_per_pixel - 1 - j;
            raw = (raw << 8) | wire[at];
        }
        pixels[x] = (unpack_channel(raw, d->red) << 16) |
                    (unpack_channel(raw, d->green) << 8) | unpack_channel(raw, d->blue);
    }
}

static int init_scanout(void)
{
    static const uint32_t black[JNOX_DISPLAY_WIDTH];
    if (jnox_display_init() < 0) return 0;
    for (unsigned y = 0; y < JNOX_DISPLAY_HEIGHT; ++y)
        if (jnox_display_write_row(y, black) < 0) return 0;
    return 1;
}

static int present_strip(struct display *d, const byte *reply)
{
    if (!d->strips_left || u16(reply + 2) != d->reply_sequence ||
        reply[1] != d->depth || u32(reply + 8) != d->visual ||
        u32(reply + 4) != (d->stride * STRIP_ROWS) / 4) return 0;
    unsigned y = TOP + HEIGHT - d->strips_left * STRIP_ROWS;
    byte wire[WIDTH * 4];
    static uint32_t pixels[JNOX_DISPLAY_WIDTH]; /* Both horizontal margins stay black. */
    for (unsigned i = 0; i < STRIP_ROWS; ++i) {
        if (!read_all(d->fd, wire, d->stride)) return 0;
        decode_row(d, wire, pixels + LEFT);
        if (jnox_display_write_row(y + i, pixels) < 0) return 0;
    }
    d->reply_sequence = (d->reply_sequence + 1) & 65535;
    --d->strips_left;
    return 1;
}

static int draw(struct display *d, unsigned clicks)
{
    return rectangle(d, 0x20b8a0, 16, 16, 64, 128) &&
           rectangle(d, 0xf4b644, 96, 16, 128, 48) &&
           rectangle(d, clicks ? 0x498cf0 : 0xe85c50, 96, 80, 128, 64) &&
           request_frame(d);
}

static int event_loop(struct display *d)
{
    unsigned clicks = 0;
    for (;;) {
        byte event[32];
        if (!read_all(d->fd, event, sizeof(event))) return 106;
        if (event[0] == 0 || (event[0] & 127) == 35) return 107;
        if (event[0] == 1) {
            if (!d->scanout || !d->strips_left) return 107;
            if (!present_strip(d, event)) return 113;
            if (!d->strips_left && d->redraw_pending && clicks < 2 && !request_frame(d))
                return 108;
            continue;
        }
        if (event[0] == EXPOSE && u32(event + 4) == d->window &&
            u16(event + 16) == 0 && clicks < 2) {
            if (!draw(d, clicks)) return 108;
        }
        if (event[0] == BUTTON_PRESS && event[1] == 1 && u32(event + 8) == d->root &&
            u32(event + 12) == d->window && event[30] && u16(event + 24) < WIDTH &&
            u16(event + 26) < HEIGHT && clicks < 2) {
            ++clicks; /* Exact event type excludes SendEvent's high bit. */
            if (clicks == 1 && !draw(d, clicks)) return 108;
            if (clicks == 2 && !window_request(d, DESTROY_WINDOW)) return 109;
        }
        if (event[0] == DESTROY_NOTIFY && u32(event + 8) == d->window)
            return clicks == 2 ? 0 : 110;
    }
}

int main(int argc, char **argv)
{
    static struct display d;
    if ((argc != 3 && argc != 4) || !argv || !argv[1] || !argv[2]) return 100;
    if (argc == 4) {
        static const char mode[] = "scanout";
        if (!argv[3]) return 100;
        for (unsigned i = 0; i < sizeof(mode); ++i)
            if (argv[3][i] != mode[i]) return 100;
        d.scanout = 1;
    }
    unsigned port = port_number(argv[1]);
    if (!port || jnox_socket_broker(BROKER) < 0) return 100;
    d.fd = socket(AF_INET, SOCK_STREAM, 0);
    if (d.fd < 0) return 101;
    struct sockaddr_in host = {0};
    host.sin_family = AF_INET;
    host.sin_addr.s_addr = htonl(0x0a000202u);
    host.sin_port = htons((in_port_t)port);
    int status;
    if (connect(d.fd, (struct sockaddr *)&host, sizeof(host)) < 0) status = 102;
    else if (!send_setup(d.fd, argv[2])) status = 103;
    else if (!read_setup(&d)) status = 104;
    else if (d.scanout && !init_scanout()) status = 112;
    else if (!open_window(&d)) status = 105;
    else status = event_loop(&d);
    return close(d.fd) < 0 && status == 0 ? 111 : status;
}
