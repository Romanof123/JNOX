/* Public SDK and Picolibc overlay can compile this unmodified caller. */
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <sys/un.h>
#include <unistd.h>

typedef int (*query_fn)(int, struct sockaddr *, socklen_t *);
static const struct sockaddr_un server = {AF_UNIX, "/xcb-server"};
static const struct sockaddr_un client = {AF_UNIX, "/xcb-client"};
static const struct sockaddr_un reused = {AF_UNIX, "/reused"};

static int listener_queue(int hint)
{
    int listener = socket(AF_UNIX, SOCK_STREAM, 0);
    const struct sockaddr *sa = (const struct sockaddr *)&server;
    if (listener < 0 || bind(listener, sa, sizeof(server)) || listen(listener, hint)) return 30;
    int clients[3];
    for (int i = 0; i < 3; ++i) {
        clients[i] = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK, 0);
        if (clients[i] < 0) return 31;
    }
    if (connect(clients[0], sa, sizeof(server)) || send(clients[0], "A", 1, 0) != 1 ||
        connect(clients[1], sa, sizeof(server)) || send(clients[1], "B", 1, 0) != 1) return 32;
    if (connect(clients[2], sa, sizeof(server)) != -1 || errno != EAGAIN) return 33;
    /* Free a descriptor while its already queued connection retains byte A. */
    if (close(clients[0])) return 34;
    for (int i = 0; i < 3; ++i) {
        int peer = accept(listener, NULL, NULL);
        char byte;
        if (peer < 0 || recv(peer, &byte, 1, 0) != 1 || byte != 'A' + i || close(peer)) return 35;
        if (i == 0 && (connect(clients[2], sa, sizeof(server)) ||
                       send(clients[2], "C", 1, 0) != 1)) return 36;
    }
    return close(clients[1]) || close(clients[2]) || close(listener) ? 37 : 0;
}

/* Compare every byte written and preserve canaries after the supplied limit.
 * An unnamed Unix address is just the family, without a pathname NUL. */
static int address(query_fn query, int fd, const struct sockaddr_un *name, socklen_t capacity)
{
    union { struct sockaddr_un align; unsigned char bytes[sizeof(struct sockaddr_un) + 8]; } storage;
    unsigned char *buf = storage.bytes;
    socklen_t expected = offsetof(struct sockaddr_un, sun_path);
    if (name->sun_path[0]) {
        while (name->sun_path[expected - offsetof(struct sockaddr_un, sun_path)]) ++expected;
        ++expected;
    }
    for (unsigned i = 0; i < sizeof(storage.bytes); ++i) buf[i] = 0xa5;
    socklen_t len = capacity;
    if (query(fd,(struct sockaddr *)(void *)buf,&len) || len != expected) return 0;
    for (unsigned i = 0; i < sizeof(storage.bytes); ++i) {
        unsigned char want = i < len && i < capacity ? ((const unsigned char *)name)[i] : 0xa5;
        if (buf[i] != want) return 0;
    }
    return 1;
}

int main(void)
{
    static const struct sockaddr_un unnamed = {AF_UNIX, ""};
    if (jnox_socket_broker(1)) return 10;
    int l = socket(AF_UNIX, SOCK_STREAM, 0), c = socket(AF_UNIX, SOCK_STREAM, 0);
    if (l < 0 || c < 0 || !address(getsockname,c,&unnamed,2)) return 11;
    struct sockaddr_un out = {0};
    socklen_t n = sizeof(out);
    if (getpeername(c,(struct sockaddr *)&out,&n) != -1 || errno != ENOTCONN ||
        n != sizeof(out) || out.sun_family) return 12;
    if (bind(l,(const struct sockaddr *)&server,sizeof(server)) || listen(l,1)) return 13;
    if (!address(getsockname,l,&server,1) || !address(getsockname,l,&server,0)) return 14;
    if (connect(c,(const struct sockaddr *)&server,sizeof(server))) return 15;
    if (accept(l,(struct sockaddr *)&out,0) != -1 || errno != EFAULT) return 38;
    out.sun_path[0] = '!';
    int a = accept(l,(struct sockaddr *)&out,&n);
    if (a < 0 || !address(getsockname,a,&server,sizeof(out)) ||
        !address(getpeername,a,&unnamed,sizeof(out)) || !address(getsockname,c,&unnamed,sizeof(out))) return 16;
    if (n != offsetof(struct sockaddr_un,sun_path) || out.sun_family != AF_UNIX ||
        out.sun_path[0] != '!') return 39;
    /* Xtrans SetOption(TRANS_NONBLOCKING) uses these two public calls. */
    int flags = fcntl(a,F_GETFL,0);
    if (flags != O_RDWR || fcntl(a,F_SETFL,flags|O_NONBLOCK) ||
        fcntl(a,F_GETFL,0) != (flags|O_NONBLOCK) || fcntl(c,F_GETFL,0) != O_RDWR) return 40;
    char empty;
    if (recv(a,&empty,1,0) != -1 || errno != EAGAIN ||
        fcntl(a,F_SETFL,flags) || fcntl(a,F_GETFL,0) != flags) return 41;
    /* Match xcb_auth.c's two-call pattern when the first buffer was too small. */
    n = 3;
    if (getpeername(c,(struct sockaddr *)&out,&n) ||
        n != offsetof(struct sockaddr_un,sun_path) + sizeof("/xcb-server") ||
        !address(getpeername,c,&server,n)) return 17;
    if (send(c,"N",1,0) != 1 || !address(getpeername,a,&unnamed,1)) return 18;
    char data;
    if (recv(a,&data,1,0) != 1 || data != 'N' || close(a)) return 19;
    int replacement = socket(AF_UNIX,SOCK_STREAM,0);
    if (replacement < 0 || bind(replacement,(const struct sockaddr *)&reused,sizeof(reused)) ||
        !address(getpeername,c,&server,sizeof(out)) || recv(c,&data,1,0) != 0) return 20;
    if (close(c) || close(replacement) || close(l)) return 21;
    if (getsockname(c,(struct sockaddr *)&out,&n) != -1 || errno != EBADF) return 22;

    l = socket(AF_UNIX,SOCK_STREAM,0);
    c = socket(AF_UNIX,SOCK_STREAM,0);
    if (l < 0 || c < 0 || bind(l,(const struct sockaddr *)&server,sizeof(server)) ||
        bind(c,(const struct sockaddr *)&client,sizeof(client)) || listen(l,1) ||
        connect(c,(const struct sockaddr *)&server,sizeof(server))) return 23;
    n = offsetof(struct sockaddr_un,sun_path) + 1;
    out.sun_path[0] = out.sun_path[1] = '!';
    a = accept(l,(struct sockaddr *)&out,&n);
    if (n != offsetof(struct sockaddr_un,sun_path) + sizeof("/xcb-client") ||
        out.sun_family != AF_UNIX || out.sun_path[0] != '/' || out.sun_path[1] != '!') return 42;
    if (a < 0 || !address(getpeername,a,&client,sizeof(out)) || close(c) ||
        !address(getpeername,a,&client,sizeof(out))) return 24;
    if (getpeername(a,0,&n) != -1 || errno != EFAULT ||
        getsockname(a,(struct sockaddr *)&out,0) != -1 || errno != EFAULT) return 25;
    if (close(a) || close(l)) return 26;
    int result = listener_queue(128); /* Xtrans MIN_BACKLOG */
    if (!result) result = listener_queue(65536); /* also exceeds the wire argument */
    return result ? result : 77;
}
