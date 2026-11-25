#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <sys/socket.h>

/* stage, outbound sent/read, first read size, EOF, inbound received/sent,
 * inbound EOF, reset send, reset read result, reset errno, clean closes. */
volatile long long inet_tcp_result[12];

static int address_call(int fd, uint32_t address, unsigned port, int listener)
{
    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    addr.sin_addr.s_addr = htonl(address);
    if (listener) return bind(fd,(struct sockaddr *)&addr,sizeof(addr));
    return connect(fd,(struct sockaddr *)&addr,sizeof(addr));
}

static int send_all(int fd, const char *s, unsigned len)
{
    unsigned done = 0;
    while (done < len) {
        ssize_t n = send(fd,s+done,len-done,0);
        if (n <= 0 || (unsigned)n > len-done) return -1;
        done += (unsigned)n;
    }
    return (int)done;
}

static int read_text(int fd,const char *text,unsigned len,unsigned cap)
{
    unsigned used = 0;
    while (used < len) {
        unsigned char b[8];
        unsigned take = len-used < cap ? len-used : cap;
        ssize_t n = recv(fd,b,take,0);
        if (n <= 0 || (unsigned)n > take) return -1;
        if (inet_tcp_result[0] == 1 && !used) inet_tcp_result[3]=n;
        for (unsigned j=0;j<(unsigned)n;++j) if (b[j]!=(unsigned char)text[used+j]) return -1;
        used += (unsigned)n;
    }
    return (int)used;
}

int main(void)
{
    const uint32_t guest = 0x0a00020fU, host = 0x0a000202U;
    if (jnox_socket_broker(2)<0) return 101;
    int listener=socket(AF_INET,SOCK_STREAM,0);
    if (listener<0 || address_call(listener,guest,43001,1)<0 || listen(listener,1)<0) return 102;
    int out=socket(AF_INET,SOCK_STREAM,0);
    if (out<0 || address_call(out,host,43002,0)<0) return 103;
    inet_tcp_result[0]=1;
    inet_tcp_result[1]=send_all(out,"GUEST/out",9);
    if (inet_tcp_result[1]!=9 || shutdown(out,SHUT_WR)<0) return 104;
    inet_tcp_result[2]=read_text(out,"HOST/outbound",13,3);
    char byte;
    inet_tcp_result[4]=recv(out,&byte,1,0);
    if (inet_tcp_result[2]!=13 || inet_tcp_result[4]!=0 || close(out)<0) return 105;
    inet_tcp_result[0]=2;
    int in=accept(listener,0,0);
    if (in<0) return 106;
    inet_tcp_result[5]=read_text(in,"HOST/inbound",12,4);
    inet_tcp_result[7]=recv(in,&byte,1,0);
    if (inet_tcp_result[5]!=12 || inet_tcp_result[7]!=0) return 107;
    inet_tcp_result[6]=send_all(in,"GUEST/accepted",14);
    if (inet_tcp_result[6]!=14 || shutdown(in,SHUT_WR)<0 || close(in)<0 || close(listener)<0) return 108;

    inet_tcp_result[0]=3;
    int reset=socket(AF_INET,SOCK_STREAM,0);
    if (reset<0 || address_call(reset,host,43002,0)<0) return 109;
    inet_tcp_result[8]=send_all(reset,"ARM!",4);
    if (inet_tcp_result[8]!=4) return 110;
    ssize_t n=recv(reset,&byte,1,0);
    int error=n<0 ? errno : 0;
    inet_tcp_result[9]=n; inet_tcp_result[10]=error;
    if (close(reset)<0) return 111;
    inet_tcp_result[11]=1;
    if (n!=-1 || error!=104) return 112;
    inet_tcp_result[0]=4;
    return 75;
}
