#include <poll.h>
#include <sys/un.h>
#include <time.h>

static const struct sockaddr_un endpoint = {AF_UNIX, "/poll"};
#define ADDRESS ((const struct sockaddr *)&endpoint)
volatile unsigned long long poll_probe[16];
/* Each PID has private storage. Rows contain the actual two pollfd entries,
 * syscall result, and start/end nanoseconds; the stage is nonzero in poll. */
volatile unsigned long long poll_stage, poll_rows[6][9], poll_io[12];
static int observed;
enum { LISTENER, TIMEOUT, WRITABLE, DATA, HANGUP, INVALID };

static unsigned long long nanos(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) return 0;
    return (unsigned long long)t.tv_sec*1000000000ULL+t.tv_nsec;
}
static int watch(unsigned stage, struct pollfd *fds, int timeout) {
    if (!observed) return poll(fds, 2, timeout);
    volatile unsigned long long *row = poll_rows[stage];
    row[1] = fds[0].fd;
    row[2] = fds[0].events;
    row[4] = fds[1].fd;
    row[5] = fds[1].events;
    row[7] = nanos();
    poll_stage = stage + 1;
    int result = poll(fds, 2, timeout);
    row[8] = nanos();
    row[0] = result;
    row[3] = fds[0].revents;
    row[6] = fds[1].revents;
    poll_stage = 0;
    return result;
}

/* Give a stopped debugger time to capture a genuinely blocked peer. */
static int observation_delay(void) {
    if (!observed) return 0;
    unsigned long long end = nanos() + 1500000000ULL;
    struct timespec when = {end / 1000000000ULL, end % 1000000000ULL};
    return clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &when, 0);
}
static int client(void) {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0 || connect(fd, ADDRESS, sizeof(endpoint))) return 24;
    char value;
    if (recv(fd,&value,1,0) != 1 || value != 'r') return 25;
    if (observed) { poll_io[0] = fd; poll_io[1] = nanos(); }
    if (observation_delay()) return 29;
    int sent = send(fd,"x",1,0);
    if (observed) { poll_io[2] = sent; poll_io[3] = nanos(); }
    if (sent != 1) return 26;
    if (recv(fd,&value,1,0) != 1 || value != 'q') return 27;
    if (observed) poll_io[4] = nanos();
    if (observation_delay()) return 30;
    int closed = close(fd);
    if (observed) { poll_io[5] = closed; poll_io[6] = nanos(); }
    return closed ? 28 : 72;
}
static int server(void) {
    int listenfd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listenfd < 0 || bind(listenfd,ADDRESS,sizeof(endpoint)) || listen(listenfd,1)) return 10;
    struct pollfd fds[] = {{listenfd,POLLIN,0},{-1,POLLIN,0}};
    if (watch(LISTENER,fds,-1) != 1 || fds[0].revents != POLLIN || fds[1].revents) return 11;
    int peer = accept(listenfd,0,0);
    if (peer < 0) return 12;
    fds[1] = (struct pollfd){peer,POLLIN,0};
    poll_probe[0] = nanos();
    if (watch(TIMEOUT,fds,observed ? 1500 : 20) || fds[0].revents || fds[1].revents) return 13;
    poll_probe[1] = nanos();
    if (poll_probe[1]-poll_probe[0] < 20000000ULL) return 14;
    fds[1].events = POLLOUT;
    if (watch(WRITABLE,fds,0) != 1 || fds[1].revents != POLLOUT) return 15;
    if (send(peer,"r",1,0) != 1) return 16;
    fds[1].events = POLLIN;
    if (watch(DATA,fds,-1) != 1 || fds[0].revents || fds[1].revents != POLLIN) return 17;
    char value;
    if (recv(peer,&value,1,0) != 1 || value != 'x' || send(peer,"q",1,0) != 1) return 18;
    fds[1].events = 0;
    if (watch(HANGUP,fds,-1) != 1 || fds[1].revents != POLLHUP) return 19;
    char eof_byte = '!';
    int eof = recv(peer, &eof_byte, 1, 0);
    if (observed) {
        poll_io[0] = peer;
        poll_io[1] = eof;
        poll_io[2] = (unsigned char)eof_byte;
        poll_io[3] = nanos();
    }
    if (eof != 0) return 22;
    if (close(peer) || watch(INVALID,fds,0) != 1 || fds[1].revents != POLLNVAL) return 20;
    poll_probe[2] = value;
    return close(listenfd) ? 21 : 71;
}
int main(int argc, char **argv) {
    if (argc != 2 || jnox_socket_broker(2)) return 9;
    observed = argv[1][0] == 'o';
    return argv[1][observed] == 's' ? server() : client();
}
