/* Public SDK and Picolibc overlay can compile this unmodified caller. */
#include <errno.h>
#include <stddef.h>
#include <sys/un.h>
#include <unistd.h>

typedef int (*query_fn)(int, struct sockaddr *, socklen_t *);
static const struct sockaddr_un server = {AF_UNIX, "/xcb-server"};
static const struct sockaddr_un client = {AF_UNIX, "/xcb-client"};
static const struct sockaddr_un reused = {AF_UNIX, "/reused"};

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
    int a = accept(l,0,0);
    if (a < 0 || !address(getsockname,a,&server,sizeof(out)) ||
        !address(getpeername,a,&unnamed,sizeof(out)) || !address(getsockname,c,&unnamed,sizeof(out))) return 16;
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
    a = accept(l,0,0);
    if (a < 0 || !address(getpeername,a,&client,sizeof(out)) || close(c) ||
        !address(getpeername,a,&client,sizeof(out))) return 24;
    if (getpeername(a,0,&n) != -1 || errno != EFAULT ||
        getsockname(a,(struct sockaddr *)&out,0) != -1 || errno != EFAULT) return 25;
    return close(a) || close(l) ? 26 : 77;
}
