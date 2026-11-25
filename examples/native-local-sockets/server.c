#include <errno.h>
#include <sys/socket.h>
#include <sys/un.h>

volatile uint64_t socket_server_result[12];
static const struct sockaddr_un gate_name = {AF_UNIX, "/gate"};
static const struct sockaddr_un data_name = {AF_UNIX, "/telemetry"};

static int mode(const char *s)
{
    const char *names[] = {"stream", "dgram", "seq"};
    const int kinds[] = {SOCK_STREAM, SOCK_DGRAM, SOCK_SEQPACKET};
    for (unsigned i = 0; i < 3; ++i) {
        unsigned j = 0;
        while (names[i][j] && names[i][j] == s[j]) ++j;
        if (!names[i][j] && !s[j]) return kinds[i];
    }
    return 0;
}

static int read_stream(int fd)
{
    unsigned char bytes[45];
    const unsigned sizes[] = {3, 16, 45};
    unsigned expected = 1;
    for (unsigned i = 0; i < 3; ++i) {
        ssize_t n = recv(fd, bytes, sizes[i], 0);
        socket_server_result[4+i] = n;
        if (n != (ssize_t)sizes[i]) return 0;
        for (unsigned j = 0; j < sizes[i]; ++j) {
            if (bytes[j] != expected++) return 0;
            socket_server_result[2] += bytes[j];
        }
        socket_server_result[3] += n;
    }
    return recv(fd,bytes,1,0) == 0;
}

static int read_packets(int fd)
{
    for (unsigned i = 1; i <= 4; ++i) {
        unsigned char bytes[8];
        ssize_t n = recv(fd,bytes,sizeof(bytes),0);
        socket_server_result[3+i] = n;
        if (n != 3) return 0;
        for (unsigned j = 0; j < 3; ++j) {
            if (bytes[j] != i + 10*j) return 0;
            socket_server_result[2] += bytes[j];
        }
        socket_server_result[3] += n;
    }
    unsigned char byte;
    return recv(fd,&byte,1,0) == 0;
}

static int read_datagrams(int fd, struct sockaddr_un *peer, socklen_t *peer_len)
{
    const char name[] = "/origin";
    unsigned char bytes[3];
    for (unsigned i = 0; i < 2; ++i) {
        *peer_len = sizeof(*peer);
        ssize_t n = recvfrom(fd,bytes,sizeof(bytes),MSG_TRUNC,(struct sockaddr *)peer,peer_len);
        socket_server_result[4+i] = n;
        if (n != (i ? 2 : 5) || peer->sun_family != AF_UNIX ||
            *peer_len != __builtin_offsetof(struct sockaddr_un,sun_path)+sizeof(name)) return 0;
        for (unsigned j = 0; j < sizeof(name); ++j)
            if (peer->sun_path[j] != name[j]) return 0;
        unsigned copied = i ? 2 : 3;
        for (unsigned j = 0; j < copied; ++j) {
            if (bytes[j] != (i ? 20u : 10u) + j) return 0;
            socket_server_result[2] += bytes[j];
        }
        socket_server_result[3] += copied;
    }
    socket_server_result[8] = *peer_len;
    return recv(fd,bytes,sizeof(bytes),MSG_DONTWAIT) == -1 && errno == EAGAIN;
}

int main(int argc, char **argv)
{
    if (argc != 2 || jnox_socket_broker(2) < 0) return 101;
    int kind = mode(argv[1]);
    if (!kind) return 102;
    socket_server_result[0] = kind;
    int gate = socket(AF_UNIX,SOCK_DGRAM,0);
    int data = socket(AF_UNIX,kind,0);
    if (gate < 0 || data < 0 ||
        bind(gate,(const struct sockaddr *)&gate_name,sizeof(gate_name)) < 0 ||
        bind(data,(const struct sockaddr *)&data_name,sizeof(data_name)) < 0) return 103;
    if (kind != SOCK_DGRAM && listen(data,2) < 0) return 104;
    unsigned char marker;
    if (recv(gate,&marker,1,0) != 1 || marker != kind) return 105;
    socket_server_result[1] = marker;
    int fd = data;
    struct sockaddr_un peer;
    socklen_t plen = 0;
    if (kind != SOCK_DGRAM) {
        fd = accept(data,0,0);
        if (fd < 0) return 106;
    }
    int ok = kind == SOCK_STREAM ? read_stream(fd) :
             kind == SOCK_SEQPACKET ? read_packets(fd) : read_datagrams(fd,&peer,&plen);
    if (!ok) return 107;
    unsigned char ack = (unsigned char)socket_server_result[2];
    ssize_t n = kind == SOCK_DGRAM ?
        sendto(fd,&ack,1,0,(struct sockaddr *)&peer,plen) : send(fd,&ack,1,0);
    if (n != 1) return 108;
    socket_server_result[9] = ack;
    if (fd != data && close(fd) < 0) return 109;
    if (close(data) < 0 || close(gate) < 0) return 110;
    socket_server_result[10] = 1;
    return 73;
}
