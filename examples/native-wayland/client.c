/* Two native Wayland clients. Each invocation owns a new connection and ID namespace. */
#include <errno.h>
#include <jnox/display.h>
#include <jnox/shm.h>
#include <time.h>
#include "protocol.h"
#include "transport.h"
#include "pixels.h"

enum { DISPLAY = 1, REGISTRY, START_SYNC, COMPOSITOR, SHM, WM, SEAT, KEYBOARD,
       SURFACE, XDG, TOPLEVEL, POOL, BUFFER, FRAME, CLOSE_SYNC, POINTER };
enum { ROLE, STAGE, FAIL_LINE, FAIL_ERRNO, DISPLAY_CODE, GLOBAL_MASK, XRGB,
       KEYMAP_SIZE, KEYMAP_HASH, CONFIGURES, ACK_SERIAL, ENTERS, LEAVES, FOCUSED,
       X_PRESSES, K_PRESSES, KEY_SERIAL, COMMITS, RELEASES, FRAMES, DRAW_HASH,
       MAP_ADDRESS, POOL_FD, TRANSFER_FD, DIED_BUSY, CLOSE_SEEN, FRAME_DELETES,
       KEY_RELEASES, DENIALS, FRAME_TIME, KEY_CODE, CLOSE_SYNC_DONE };
volatile uint64_t wayland_client_witness[32];
#define W wayland_client_witness
static int failed(unsigned line)
{
    if (!W[FAIL_LINE]) { W[FAIL_LINE] = line; W[FAIL_ERRNO] = errno; }
    return 0;
}
#define REQUIRE(expr) do { if (!(expr)) return failed(__LINE__); } while (0)

static const char *const interfaces[] = {"wl_compositor", "wl_shm", "xdg_wm_base", "wl_seat"};
static uint32_t globals[4];
static int tiles, start_done, caps, keyboard_requested, surface_created, toplevel_configured;
enum { P_CAPS, P_ENTERS, P_LEAVES, P_MOTIONS, P_PRESSES, P_RELEASES,
       P_X_FIXED, P_Y_FIXED, P_ENTER_SERIAL, P_BUTTON_SERIAL, P_RESOURCE_RELEASED };
volatile uint64_t wayland_pointer_witness[11];
#define P wayland_pointer_witness
static int pointer_profile, pointer_requested, pointer_focused, button_down;
static int busy, frame_active, frame_done, damage_pending, die_after_commit, closing;
static volatile uint32_t *mapping;
static const struct sockaddr_un endpoint = {AF_UNIX, "/wayland-0"};

static int connect_display(void)
{
    REQUIRE(jnox_socket_broker(3) == 0);
    channel = socket(AF_UNIX, SOCK_STREAM, 0);
    REQUIRE(channel >= 0);
    while (connect(channel, (const struct sockaddr *)&endpoint, sizeof(endpoint)) < 0) {
        REQUIRE(errno == ENOENT || errno == ECONNREFUSED || errno == EAGAIN);
        struct timespec until;
        REQUIRE(clock_gettime(CLOCK_MONOTONIC, &until) == 0);
        until.tv_nsec += 10000000;
        if (until.tv_nsec >= 1000000000) { ++until.tv_sec; until.tv_nsec -= 1000000000; }
        REQUIRE(clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &until, 0) == 0);
    }
    W[STAGE] = 1;
    return 1;
}

static int registry_global(const uint32_t *a, unsigned n)
{
    REQUIRE(n >= 4 && a[1] > 0 && a[1] <= (n-3)*4);
    unsigned bytes = a[1], cells = (bytes + 3)/4;
    REQUIRE(n == 3 + cells && ((const char *)(a+2))[bytes-1] == 0);
    for (unsigned i = 0; i < 4; ++i) {
        const char *expected = interfaces[i], *name = (const char *)(a+2);
        unsigned j = 0;
        while (j+1 < bytes && expected[j] && expected[j] == name[j]) ++j;
        if (j+1 == bytes && !expected[j]) {
            REQUIRE(a[0] && a[2+cells] >= 1 && !globals[i]);
            globals[i] = a[0];
            W[GLOBAL_MASK] |= 1u << i;
        }
    }
    return 1;
}

static int bind_globals(void)
{
    for (unsigned i = 0; i < 4; ++i) {
        uint32_t args[16];
        args[0] = globals[i];
        unsigned n = pack_string(args+1, 13, interfaces[i]);
        REQUIRE(n);
        args[1+n] = i == 3 && pointer_profile ? 3 : 1; /* requested version */
        args[2+n] = COMPOSITOR+i; /* dynamic new_id includes interface and version */
        REQUIRE(send_wire(REGISTRY, REGISTRY_BIND, args, 3+n, -1));
    }
    W[STAGE] = 2;
    return 1;
}

static int read_keymap(const uint32_t *args, unsigned n)
{
    REQUIRE(n == 2 && args[0] == KEYMAP_XKB_V1 && args[1] >= 11 && args[1] <= JNOX_SHM_MAX_BYTES);
    int fd = take_fd();
    REQUIRE(fd >= 0 && jnox_shm_size(fd) >= (long)args[1]);
    const volatile unsigned char *text = jnox_shm_map(fd, JNOX_SHM_READ);
    REQUIRE(text != JNOX_SHM_MAP_FAILED && text[args[1]-1] == 0);
    const char prefix[] = "xkb_keymap";
    for (unsigned i = 0; i < sizeof(prefix)-1; ++i) REQUIRE(text[i] == (unsigned char)prefix[i]);
    uint32_t hash = 2166136261u;
    for (unsigned i = 0; i < args[1]; ++i) hash = (hash ^ text[i]) * 16777619u;
    W[KEYMAP_SIZE] = args[1];
    W[KEYMAP_HASH] = hash;
    REQUIRE(jnox_shm_unmap((void *)text) == 0 && jnox_shm_close(fd) == 0);
    return 1;
}

static int create_surface(void)
{
    REQUIRE(WORD_REQUEST(COMPOSITOR, COMPOSITOR_CREATE_SURFACE, SURFACE));
    REQUIRE(WORD_REQUEST(WM, WM_GET_XDG_SURFACE, XDG, SURFACE));
    REQUIRE(WORD_REQUEST(XDG, XDG_GET_TOPLEVEL, TOPLEVEL));
    REQUIRE(string_request(TOPLEVEL, TOPLEVEL_SET_TITLE, tiles ? "Tiles" : "Pulse"));
    REQUIRE(string_request(TOPLEVEL, TOPLEVEL_SET_APP_ID, tiles ? "org.jnox.Tiles" : "org.jnox.Pulse"));
    REQUIRE(send_wire(SURFACE, SURFACE_COMMIT, 0, 0, -1));
    surface_created = 1;
    return 1;
}

static int create_buffer(void)
{
    int fd = jnox_shm_create(POOL_BYTES);
    REQUIRE(fd >= 0);
    W[POOL_FD] = fd;
    mapping = jnox_shm_map(fd, JNOX_SHM_READ | JNOX_SHM_WRITE);
    REQUIRE(mapping != JNOX_SHM_MAP_FAILED);
    W[MAP_ADDRESS] = (uintptr_t)mapping;
    int ro = jnox_shm_dup_readonly(fd);
    REQUIRE(ro >= 0);
    W[TRANSFER_FD] = ro;
    REQUIRE(send_wire(SHM, SHM_CREATE_POOL, (uint32_t[]){POOL, POOL_BYTES}, 2, ro));
    REQUIRE(jnox_shm_close(ro) == 0 && jnox_shm_close(fd) == 0);
    REQUIRE(WORD_REQUEST(POOL, POOL_CREATE_BUFFER, BUFFER, 0, WIDTH, HEIGHT, STRIDE, FORMAT_XRGB8888));
    return 1;
}

static int redraw(void)
{
    REQUIRE(!busy && !frame_active && mapping && W[CONFIGURES]);
    W[DRAW_HASH] = paint(mapping, tiles, (unsigned)(W[X_PRESSES] + P[P_PRESSES]));
    REQUIRE(WORD_REQUEST(SURFACE, SURFACE_ATTACH, BUFFER, 0, 0));
    REQUIRE(WORD_REQUEST(SURFACE, SURFACE_DAMAGE, 0, 0, WIDTH, HEIGHT));
    REQUIRE(WORD_REQUEST(SURFACE, SURFACE_FRAME, FRAME));
    REQUIRE(send_wire(SURFACE, SURFACE_COMMIT, 0, 0, -1));
    busy = frame_active = 1;
    damage_pending = frame_done = 0;
    ++W[COMMITS];
    W[STAGE] = 4;
    return 1;
}

static int keyboard_event(unsigned op, const uint32_t *a, unsigned n)
{
    if (op == KEYBOARD_KEYMAP) return read_keymap(a, n);
    if (op == KEYBOARD_ENTER) {
        REQUIRE(n >= 3 && a[1] == SURFACE && a[2] == (n-3)*4 && !W[FOCUSED]);
        W[FOCUSED] = 1;
        ++W[ENTERS];
    } else if (op == KEYBOARD_LEAVE) {
        REQUIRE(n == 2 && a[1] == SURFACE && W[FOCUSED]);
        W[FOCUSED] = 0;
        ++W[LEAVES];
    } else if (op == KEYBOARD_MODIFIERS) {
        REQUIRE(n == 5);
    } else if (op == KEYBOARD_KEY) {
        REQUIRE(n == 4 && a[3] <= 1 && W[FOCUSED]);
        if (!a[3]) { ++W[KEY_RELEASES]; return 1; }
        W[KEY_SERIAL] = a[0];
        W[KEY_CODE] = a[2];
        if (a[2] == 45) { ++W[X_PRESSES]; damage_pending = 1; } /* Linux KEY_X */
        if (a[2] == 37 && tiles) { /* KEY_K: die with a submitted buffer */
            ++W[K_PRESSES];
            damage_pending = die_after_commit = 1;
        }
    } else return failed(__LINE__);
    return 1;
}

static int event(void)
{
    uint32_t header[2], a[62];
    REQUIRE(read_wire(header, sizeof(header)));
    unsigned obj = header[0], size = header[1] >> 16, op = header[1] & 65535;
    REQUIRE(size >= 8 && size <= 8 + sizeof(a) && size % 4 == 0);
    unsigned n = (size-8)/4;
    REQUIRE(read_wire(a, size-8));
    switch (obj) {
    case DISPLAY:
        if (op == DISPLAY_ERROR && n >= 2) { W[DISPLAY_CODE] = a[1]; return failed(__LINE__); }
        REQUIRE(op == DISPLAY_DELETE_ID && n == 1);
        if (a[0] == POINTER) { pointer_requested = 0; ++P[P_RESOURCE_RELEASED]; }
        if (a[0] == FRAME) {
            REQUIRE(frame_active && frame_done);
            frame_active = 0;
            ++W[FRAME_DELETES];
        }
        break;
    case REGISTRY:
        if (op == REGISTRY_GLOBAL) return registry_global(a, n);
        REQUIRE(op == REGISTRY_GLOBAL_REMOVE && n == 1);
        break;
    case START_SYNC:
    case FRAME:
    case CLOSE_SYNC:
        REQUIRE(op == CALLBACK_DONE && n == 1);
        if (obj == START_SYNC) { REQUIRE(!start_done); start_done = 1; }
        else if (obj == CLOSE_SYNC) { REQUIRE(closing); W[CLOSE_SYNC_DONE] = 1; }
        else {
            REQUIRE(frame_active && !frame_done);
            frame_done = 1;
            W[FRAME_TIME] = a[0];
            ++W[FRAMES];
        }
        break;
    case SHM:
        REQUIRE(op == SHM_FORMAT && n == 1);
        if (a[0] == FORMAT_XRGB8888) W[XRGB] = 1;
        break;
    case POINTER:
        REQUIRE(pointer_requested);
        if (op == 0) {
            REQUIRE(n == 4 && a[1] == SURFACE && !pointer_focused);
            pointer_focused = 1;
            ++P[P_ENTERS];
            P[P_ENTER_SERIAL] = a[0];
            P[P_X_FIXED] = a[2]; P[P_Y_FIXED] = a[3];
        } else if (op == 1) {
            REQUIRE(n == 2 && a[1] == SURFACE && pointer_focused);
            pointer_focused = button_down = 0;
            ++P[P_LEAVES];
        } else if (op == 2) {
            REQUIRE(n == 3 && pointer_focused);
            ++P[P_MOTIONS];
            P[P_X_FIXED] = a[1]; P[P_Y_FIXED] = a[2];
        } else {
            REQUIRE(op == 3 && n == 4 && pointer_focused && a[2] == 272 && a[3] <= 1);
            REQUIRE(button_down != (int)a[3]);
            button_down = a[3];
            P[P_BUTTON_SERIAL] = a[0];
            if (button_down) { ++P[P_PRESSES]; damage_pending = 1; }
            else ++P[P_RELEASES];
        }
        break;
    case SEAT:
        if (op == 1 && pointer_profile) { REQUIRE(n == 3 && a[0] == 6); break; }
        REQUIRE(op == SEAT_CAPABILITIES && n == 1);
        caps = !!(a[0] & CAP_KEYBOARD);
        if (pointer_profile) {
            P[P_CAPS] = a[0];
            if ((a[0] & CAP_POINTER) && !pointer_requested) {
                REQUIRE(WORD_REQUEST(SEAT, SEAT_GET_POINTER, POINTER));
                pointer_requested = 1;
            } else if (!(a[0] & CAP_POINTER) && pointer_requested) {
                REQUIRE(send_wire(POINTER, POINTER_RELEASE, 0, 0, -1));
            }
        }
        break;
    case KEYBOARD:
        REQUIRE(keyboard_requested);
        return keyboard_event(op, a, n);
    case WM:
        REQUIRE(op == WM_PING && n == 1 && WORD_REQUEST(WM, WM_PONG, a[0]));
        break;
    case TOPLEVEL:
        if (op == TOPLEVEL_CLOSE) { REQUIRE(n == 0); W[CLOSE_SEEN] = 1; }
        else {
            REQUIRE(op == TOPLEVEL_CONFIGURE && n >= 3 && a[2] == (n-3)*4);
            toplevel_configured = 1;
        }
        break;
    case XDG:
        REQUIRE(op == XDG_CONFIGURE && n == 1 && surface_created && toplevel_configured);
        REQUIRE(WORD_REQUEST(XDG, XDG_ACK_CONFIGURE, a[0]));
        W[ACK_SERIAL] = a[0];
        ++W[CONFIGURES];
        break;
    case BUFFER:
        REQUIRE(op == BUFFER_RELEASE && n == 0 && busy);
        busy = 0;
        ++W[RELEASES];
        break;
    default:
        return failed(__LINE__);
    }
    return 1;
}

static int shutdown_client(void)
{
    closing = 1;
    if (pointer_requested && (P[P_CAPS] & CAP_POINTER))
        REQUIRE(send_wire(POINTER, POINTER_RELEASE, 0, 0, -1));
    REQUIRE(send_wire(TOPLEVEL, TOPLEVEL_DESTROY, 0, 0, -1));
    REQUIRE(send_wire(XDG, XDG_DESTROY, 0, 0, -1));
    REQUIRE(send_wire(SURFACE, SURFACE_DESTROY, 0, 0, -1));
    REQUIRE(send_wire(BUFFER, BUFFER_DESTROY, 0, 0, -1));
    REQUIRE(send_wire(POOL, POOL_DESTROY, 0, 0, -1));
    REQUIRE(send_wire(WM, WM_DESTROY, 0, 0, -1));
    REQUIRE(WORD_REQUEST(DISPLAY, DISPLAY_SYNC, CLOSE_SYNC));
    while (!W[CLOSE_SYNC_DONE]) REQUIRE(event());
    REQUIRE(!queued_count && jnox_shm_unmap((void *)mapping) == 0 && close(channel) == 0);
    W[STAGE] = 6;
    return 1;
}

static int run(void)
{
    static const uint32_t row[JNOX_DISPLAY_WIDTH];
    REQUIRE(jnox_display_init() == -EPERM && jnox_display_write_row(0, row) == -EPERM &&
            jnox_display_poll_key() == -EPERM);
    W[DENIALS] = 3;
    if (pointer_profile) {
        REQUIRE(jnox_display_call(197, 0, 0) == -EPERM);
        REQUIRE(jnox_display_call(197, 1, 0) == -EPERM);
        W[DENIALS] += 2;
    }
    REQUIRE(connect_display());
    REQUIRE(WORD_REQUEST(DISPLAY, DISPLAY_GET_REGISTRY, REGISTRY));
    REQUIRE(WORD_REQUEST(DISPLAY, DISPLAY_SYNC, START_SYNC));
    while (!start_done) REQUIRE(event());
    REQUIRE(W[GLOBAL_MASK] == 15 && bind_globals());
    while (!W[XRGB] || !caps) REQUIRE(event());
    REQUIRE(WORD_REQUEST(SEAT, SEAT_GET_KEYBOARD, KEYBOARD));
    keyboard_requested = 1;
    while (!W[KEYMAP_SIZE]) REQUIRE(event());
    REQUIRE(create_surface());
    while (!W[CONFIGURES]) REQUIRE(event());
    W[STAGE] = 3;
    REQUIRE(create_buffer() && redraw());
    for (;;) {
        REQUIRE(event());
        if (W[CLOSE_SEEN] && !busy && !frame_active) return shutdown_client();
        if (damage_pending && !busy && !frame_active) {
            REQUIRE(redraw());
            if (die_after_commit) {
                W[DIED_BUSY] = busy;
                W[STAGE] = 5;
                /* Deliberately leave the mapping, socket and protocol objects live. */
                return 1;
            }
        }
    }
}

int main(int argc, char **argv)
{
    if (argc != 2 || !argv[1]) return 99;
    const char *role = argv[1];
    if (*role == 'p') { pointer_profile = 1; ++role; }
    if ((role[0] != 'a' && role[0] != 'b') || role[1]) return 99;
    tiles = role[0] == 'b';
    W[ROLE] = tiles ? 2 : 1;
    if (!run()) return 100;
    return tiles ? 72 : 71;
}
