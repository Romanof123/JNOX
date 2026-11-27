/* Test only the X11 setup authentication boundary on the local stream. */
#include <errno.h>
#include <string.h>
#include <sys/un.h>
#include <unistd.h>
#include <jnox/xauthority_stream.h>

extern void InitAuthorization(const char *);
extern void InitNativeAuthorization(void);
extern unsigned long CheckAuthorization(unsigned int, const char *, unsigned int,
                                        const char *, void *, const char **);
extern int stream_probe(void);

static int yield_guest(void)
{
    return (int)jnox_ipc_trap(8, 0, 0, 0).status;
}

int main(void)
{
    InitAuthorization("/run/Xauthority");
    InitNativeAuthorization();
    if (stream_probe()) return 92;
    if (jnox_socket_broker(3)) return 93;
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un addr = {AF_UNIX, "/tmp/.X11-unix/X0"};
    if (listener < 0 || bind(listener, (struct sockaddr *)&addr, sizeof(addr)) ||
        listen(listener, 1)) return 94;
    int client;
    while ((client = accept(listener, NULL, NULL)) < 0) {
        if (errno != EAGAIN || yield_guest()) return 95;
    }
    unsigned char request[48];
    size_t used = 0;
    while (used < sizeof(request)) {
        ssize_t n = recv(client, request + used, sizeof(request) - used, 0);
        if (n < 0 && errno == EAGAIN && !yield_guest()) continue;
        if (n <= 0) return 96;
        used += (size_t)n;
    }
    /* Mosaic sends little-endian X11.0, an 18-byte name and 16-byte cookie. */
    if (request[0] != 'l' || request[2] != 11 || request[3] ||
        request[4] || request[5] || request[6] != 18 || request[7] ||
        request[8] != 16 || request[9]) return 97;
    const char *reason = NULL;
    unsigned long id = CheckAuthorization(18, (char *)request + 12, 16,
                                          (char *)request + 32, NULL, &reason);
    jnox_xauthority_wipe(request, sizeof(request));
    if (id != ~0UL) {
        close(client);
        close(listener);
        return 70;
    }
    if (!reason || strcmp(reason, "Invalid MIT-MAGIC-COOKIE-1 key")) return 98;
    size_t len = strlen(reason), padded = (len + 3) & ~(size_t)3;
    unsigned char reply[64] = {0};
    reply[1] = len;
    reply[2] = 11;
    reply[6] = padded / 4;
    memcpy(reply + 8, reason, len);
    if (send(client, reply, padded + 8, 0) != (ssize_t)(padded + 8)) return 99;
    close(client);
    close(listener);
    return 71;
}
