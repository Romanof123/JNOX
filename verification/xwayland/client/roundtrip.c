/* Exercise the installed upstream registry, proxy, dispatch and sync implementations. */
#ifdef JNOX_REGISTRY_ENV_CONNECT
#include <stdlib.h>
#endif
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <wayland-client.h>

struct global {
    const char *interface;
    uint32_t version;
    uint32_t name;
    int optional;
};

static struct global globals[] = {
    {"wl_compositor", 4, 0, 0}, {"wl_shm", 1, 0, 0},
    {"xdg_wm_base", 1, 0, 0}, {"wl_seat", 4, 0, 0},
    {"wl_output", 3, 0, 1}
};
static int invalid;

extern int jnox_graphics_emit(const char *, size_t);

static void advertised(void *data, struct wl_registry *registry, uint32_t name,
                       const char *interface, uint32_t version)
{
    (void)data;
    (void)registry;
    for (size_t i = 0; i < sizeof(globals) / sizeof(globals[0]); i++) {
        struct global *g = &globals[i];
        if (strcmp(interface, g->interface) == 0) {
            if (!name || g->name || version != g->version)
                invalid = 1;
            g->name = name;
            break;
        }
    }
    char line[96];
    int length = snprintf(line, sizeof(line), "JNOX_REGISTRY global %s %lu %lu\n",
                          interface, (unsigned long)name, (unsigned long)version);
    if (length < 0 || (size_t)length >= sizeof(line) ||
        jnox_graphics_emit(line, (size_t)length))
        invalid = 1;
}

static void removed(void *data, struct wl_registry *registry, uint32_t name)
{
    (void)data;
    (void)registry;
    for (size_t i = 0; i < sizeof(globals) / sizeof(globals[0]); i++)
        if (globals[i].name == name)
            invalid = 1;
}

static const struct wl_registry_listener listener = {advertised, removed};

struct output_state {
    struct wl_output *proxy;
    unsigned stage;
    int failed;
};

static void output_step(struct output_state *s, struct wl_output *proxy,
                        unsigned expected, int fields_match)
{
    if (proxy != s->proxy || s->stage != expected || !fields_match)
        s->failed = 1;
    s->stage++;
}

static void geometry(void *data, struct wl_output *proxy, int32_t x, int32_t y,
                     int32_t mm_w, int32_t mm_h, int32_t subpixel, const char *make,
                     const char *model, int32_t transform)
{
    output_step(data, proxy, 0, x == 0 && y == 0 && mm_w == 0 && mm_h == 0 &&
                subpixel == WL_OUTPUT_SUBPIXEL_UNKNOWN && !strcmp(make, "JNOX") &&
                !strcmp(model, "RAM framebuffer") && transform == WL_OUTPUT_TRANSFORM_NORMAL);
}

static void mode(void *data, struct wl_output *proxy, uint32_t flags,
                 int32_t width, int32_t height, int32_t refresh)
{
    output_step(data, proxy, 1, flags == (WL_OUTPUT_MODE_CURRENT | WL_OUTPUT_MODE_PREFERRED) &&
                width == 320 && height == 240 && refresh == 0);
}

static void scale(void *data, struct wl_output *proxy, int32_t factor)
{
    output_step(data, proxy, 2, factor == 1);
}

static void done(void *data, struct wl_output *proxy)
{
    output_step(data, proxy, 3, 1);
}

static const struct wl_output_listener output_events = {
    .geometry = geometry, .mode = mode, .scale = scale, .done = done
};

static int check_output(struct wl_display *display, struct wl_registry *registry, uint32_t name)
{
    /* Release is a v3 destructor. The second bind exercises the same stream
     * after a synchronized destruction without referencing the freed proxy. */
    for (unsigned pass = 1; pass <= 2; pass++) {
        struct output_state state = {0};
        state.proxy = wl_registry_bind(registry, name, &wl_output_interface, 3);
        if (!state.proxy)
            return 16;
        int ok = wl_output_add_listener(state.proxy, &output_events, &state) == 0 &&
                 wl_display_roundtrip(display) >= 0 && !state.failed && state.stage == 4;
        wl_output_release(state.proxy);
        if (!ok || wl_display_roundtrip(display) < 0)
            return 17;
        char line[100];
        int n = snprintf(line, sizeof(line),
                         "JNOX_OUTPUT v3 pass %u geometry-ok mode 320x240 scale 1 done released\n", pass);
        if (n < 0 || (size_t)n >= sizeof(line) || jnox_graphics_emit(line, (size_t)n))
            return 18;
    }
    return 0;
}

static int run(void)
{
#ifdef JNOX_REGISTRY_ENV_CONNECT
    const char *name = getenv("WAYLAND_DISPLAY");
    if (!name || strcmp(name, "/wayland-0") || getenv("WAYLAND_SOCKET"))
        return 11;
    if (jnox_socket_broker(3) < 0)
        return 1;
    struct wl_display *absent = wl_display_connect("/jnox-absent");
    if (absent) {
        wl_display_disconnect(absent);
        return 12;
    }
    if (errno != ENOENT && errno != ECONNREFUSED)
        return 13;
    const int missing_errno = errno;
    struct wl_display *display;
    for (unsigned attempt = 0; !(display = wl_display_connect(NULL)); attempt++) {
        if (attempt == 100 || (errno != ENOENT && errno != ECONNREFUSED && errno != EAGAIN))
            return 3;
        if (poll(NULL, 0, 10) < 0)
            return 4;
    }
    int fd = wl_display_get_fd(display);
    if (fd < 0 || fcntl(fd, F_GETFD) != FD_CLOEXEC) {
        wl_display_disconnect(display);
        return 14;
    }
    char connection[96];
    int count = snprintf(connection, sizeof(connection),
                         "JNOX_REGISTRY env-connect %s fd %d absent-errno %d\n",
                         name, fd, missing_errno);
    if (count <= 0 || (size_t)count >= sizeof(connection) ||
        jnox_graphics_emit(connection, (size_t)count)) {
        wl_display_disconnect(display);
        return 15;
    }
#else
    struct sockaddr_un address = {AF_UNIX, "/wayland-0"};
    if (jnox_socket_broker(3) < 0)
        return 1;
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0)
        return 2;
    for (unsigned attempt = 0; connect(fd, (struct sockaddr *)&address,
             offsetof(struct sockaddr_un, sun_path) + sizeof("/wayland-0")) < 0; attempt++) {
        if (attempt == 100 || (errno != ENOENT && errno != ECONNREFUSED && errno != EAGAIN)) {
            close(fd);
            return 3;
        }
        if (poll(NULL, 0, 10) < 0) {
            close(fd);
            return 4;
        }
    }
    struct wl_display *display = wl_display_connect_to_fd(fd);
    if (!display)
        return 5;
#endif
    int result = 0;
    struct wl_registry *registry = wl_display_get_registry(display);
    if (!registry || wl_registry_add_listener(registry, &listener, NULL) < 0)
        result = 6;
    else if (wl_display_roundtrip(display) < 0)
        result = 7;
    else {
        for (size_t i = 0; i < sizeof(globals) / sizeof(globals[0]); i++) {
            if (!globals[i].name) {
                if (!globals[i].optional)
                    invalid = 1;
                continue;
            }
            for (size_t j = 0; j < i; j++)
                if (globals[i].name == globals[j].name)
                    invalid = 1;
        }
        if (invalid)
            result = 8;
        else {
            for (size_t i = 0; i < sizeof(globals) / sizeof(globals[0]); i++)
                if (globals[i].name && !strcmp(globals[i].interface, "wl_output"))
                    result = check_output(display, registry, globals[i].name);
        }
    }
    if (registry)
        wl_registry_destroy(registry);
    wl_display_disconnect(display);
    if (fcntl(fd, F_GETFD) != -1 || errno != EBADF)
        return 9;
    return result;
}

int main(int argc, char **argv)
{
    if (argc == 2 && strcmp(argv[1], "idle") == 0)
        return 0;
    int status = run();
    char line[64];
    int length = snprintf(line, sizeof(line), "JNOX_REGISTRY disconnect-result %d\n", status);
    if (length <= 0 || (size_t)length >= sizeof(line) || jnox_graphics_emit(line, (size_t)length))
        return 10;
    return status;
}
