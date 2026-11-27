#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <string.h>
#include <sys/utsname.h>
#include <unistd.h>
extern int _XimXTransGetHostname(char *, int);

int main(int argc, char **argv)
{
    struct utsname u;
    char buf[65];
    if (argc != 2) return 20;
    if (!strcmp(argv[1], "absent")) {
        if (uname(&u) != -1 || errno != ENODATA) return 21;
        if (gethostname(buf, sizeof buf) != -1 || errno != ENODATA) return 22;
        buf[0] = 1;
        if (_XimXTransGetHostname(buf,sizeof buf) != -1 || buf[0] || errno != ENODATA) return 27;
        return 0;
    }
    if (uname(&u) || strcmp(u.sysname,"JNOX") || strcmp(u.machine,"aarch64") ||
        strcmp(u.release,"0.1.0") || strcmp(u.version,"jnox-kernel 0.1.0")) return 23;
    if (strcmp(u.nodename, argv[1]) || gethostname(buf,sizeof buf) || strcmp(buf,argv[1])) return 24;
    size_t n = strlen(argv[1]);
    memset(buf,1,sizeof buf);
    if (gethostname(buf,n) != -1 || errno != ENAMETOOLONG || buf[0] != 1) return 25;
    if (gethostname(buf,n+1) || strcmp(buf,argv[1])) return 26;
    if (_XimXTransGetHostname(buf,sizeof buf) != (int)n || strcmp(buf,argv[1])) return 28;
    if (_XimXTransGetHostname(buf,3) != 2 || strncmp(buf,argv[1],2) || buf[2]) return 29;
    return 0;
}
