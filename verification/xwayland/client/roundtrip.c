/* Exercise the installed upstream registry, proxy, dispatch and sync implementations. */
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
};

static struct global globals[] = {
    {"wl_compositor", 4, 0}, {"wl_shm", 1, 0},
    {"xdg_wm_base", 1, 0}, {"wl_seat", 4, 0}
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
            char line[96];
            int length = snprintf(line, sizeof(line), "JNOX_REGISTRY global %s %lu %lu\n",
                                  interface, (unsigned long)name, (unsigned long)version);
            if (length < 0 || (size_t)length >= sizeof(line) ||
                jnox_graphics_emit(line, (size_t)length))
                invalid = 1;
        }
    }
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

static int run(void)
{
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
    int result = 0;
    struct wl_registry *registry = wl_display_get_registry(display);
    if (!registry || wl_registry_add_listener(registry, &listener, NULL) < 0)
        result = 6;
    else if (wl_display_roundtrip(display) < 0)
        result = 7;
    else {
        for (size_t i = 0; i < sizeof(globals) / sizeof(globals[0]); i++)
            for (size_t j = 0; j <= i; j++)
                if (!globals[i].name || (i != j && globals[i].name == globals[j].name))
                    invalid = 1;
        if (invalid)
            result = 8;
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
