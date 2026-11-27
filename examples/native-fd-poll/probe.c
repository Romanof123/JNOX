#include <poll.h>
#include <sys/un.h>
#include <time.h>

static const struct sockaddr_un endpoint = {AF_UNIX, "/poll"};
#define ADDRESS ((const struct sockaddr *)&endpoint)
volatile unsigned long long poll_probe[16];

static unsigned long long nanos(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return 0;
    return (unsigned long long)t.tv_sec*1000000000ULL+t.tv_nsec;
}
static int client(void) {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0 || connect(fd, ADDRESS, sizeof(endpoint))) return 24;
    char value;
    if (recv(fd,&value,1,0) != 1 || value != 'r') return 25;
    if (send(fd,"x",1,0) != 1) return 26;
    if (recv(fd,&value,1,0) != 1 || value != 'q') return 27;
    return close(fd) ? 28 : 72;
}
static int server(void) {
    int listenfd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listenfd < 0 || bind(listenfd,ADDRESS,sizeof(endpoint)) || listen(listenfd,1)) return 10;
    struct pollfd fds[] = {{listenfd,POLLIN,0},{-1,POLLIN,0}};
    if (poll(fds,2,-1) != 1 || fds[0].revents != POLLIN || fds[1].revents) return 11;
    int peer = accept(listenfd,0,0);
    if (peer < 0) return 12;
    fds[1] = (struct pollfd){peer,POLLIN,0};
    poll_probe[0] = nanos();
    if (poll(fds,2,20) || fds[0].revents || fds[1].revents) return 13;
    poll_probe[1] = nanos();
    if (poll_probe[1]-poll_probe[0] < 20000000ULL) return 14;
    fds[1].events = POLLOUT;
    if (poll(fds,2,0) != 1 || fds[1].revents != POLLOUT) return 15;
    if (send(peer,"r",1,0) != 1) return 16;
    fds[1].events = POLLIN;
    if (poll(fds,2,-1) != 1 || fds[0].revents || fds[1].revents != POLLIN) return 17;
    char value;
    if (recv(peer,&value,1,0) != 1 || value != 'x' || send(peer,"q",1,0) != 1) return 18;
    fds[1].events = 0;
    if (poll(fds,2,-1) != 1 || fds[1].revents != POLLHUP) return 19;
    if (close(peer) || poll(fds,2,0) != 1 || fds[1].revents != POLLNVAL) return 20;
    poll_probe[2] = value;
    return close(listenfd) ? 21 : 71;
}
int main(int argc, char **argv) {
    if (argc != 2 || jnox_socket_broker(2)) return 9;
    return argv[1][0] == 's' ? server() : client();
}
