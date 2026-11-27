/* An ordinary packaged process. The same executable runs in both private slots. */
#include <stddef.h>
#include <stdint.h>
#ifdef CHECK_PICOLIBC
#include <stdlib.h>
extern char **environ;
extern int jnox_graphics_emit(const char *, size_t);
#else
#include <unistd.h>
#endif

static int same(const char *a, const char *b)
{
    if (!a || !b) return a == b;
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

/* Include the terminating NUL from each actual target string. */
static uint32_t digest(const char *s, uint32_t h)
{
    do {
        h = (h ^ (unsigned char)*s) * 16777619u;
    } while (*s++);
    return h;
}

static int receipt(char phase, char role, unsigned count, uint32_t hash)
{
    static const char hex[] = "0123456789abcdef";
    char line[] = "JNOX ENV ? ? ? 00000000\n";
    line[9] = phase;
    line[11] = role;
    line[13] = '0' + count;
    for (unsigned i = 0; i < 8; ++i)
        line[15+i] = hex[(hash >> (28-4*i)) & 15];
#ifdef CHECK_PICOLIBC
    return jnox_graphics_emit(line, sizeof(line)-1) == 0;
#else
    return write(1, line, sizeof(line)-1) == (long)(sizeof(line)-1);
#endif
}

#ifdef CHECK_PICOLIBC
static char **initial_environ;
static char initial_role;
static unsigned constructor_count;
static uint32_t constructor_hash;

__attribute__((constructor,noinline)) void env_constructor(void)
{
    ++constructor_count;
    initial_environ = environ;
    if (!environ) return;
    const char *display = getenv("WAYLAND_DISPLAY");
    const char *path = getenv("XDG_RUNTIME_DIR");
    const char *option = getenv("OPTION");
    const char *empty = getenv("EMPTY");
    if (getenv("ABSENT") || getenv("WAYLAND") || getenv("XDG_RUNTIME")) return;
    if (!display && !path && !option && !empty && !environ[0]) {
        initial_role = '-';
    } else if (same(display, "wayland-a") && same(path, "/run/user/A") &&
               same(option, "x=y") && same(empty, "")) {
        initial_role = 'A';
    } else if (same(display, "wayland-b") && same(path, "/run/user/B") &&
               same(option, "p q=r") && same(empty, "")) {
        initial_role = 'B';
    } else {
        return;
    }
    constructor_hash = 2166136261u;
    unsigned n = 0;
    while (n < 4 && environ[n]) constructor_hash = digest(environ[n++], constructor_hash);
    if (environ[n] || !receipt('C',initial_role,n,constructor_hash)) initial_role = 0;
}
#endif

int main(int argc, char **argv, char **envp)
{
    if (argc != 2 || !argv || !same(argv[0], "/bin/app") || argv[2] ||
        !envp || envp != argv + argc + 1) return 100;
    int first = same(argv[1],"A") || same(argv[1],"empty-A");
    int empty = same(argv[1],"empty-A") || same(argv[1],"empty-B");
    if (!(first || same(argv[1],"B") || same(argv[1],"empty-B"))) return 101;
    char role = first ? 'A' : 'B';
    static const char *const a[] = {
        "WAYLAND_DISPLAY=wayland-a", "XDG_RUNTIME_DIR=/run/user/A", "EMPTY=", "OPTION=x=y"
    };
    static const char *const b[] = {
        "XDG_RUNTIME_DIR=/run/user/B", "OPTION=p q=r", "WAYLAND_DISPLAY=wayland-b", "EMPTY="
    };
    const char *const *expected = first ? a : b;
    unsigned count = empty ? 0 : 4;
    uint32_t hash = 2166136261u;
    for (unsigned i = 0; i < count; ++i) {
        if (!same(envp[i],expected[i])) return 102+i;
        hash = digest(envp[i],hash);
    }
    if (envp[count]) return 106;
#ifdef CHECK_PICOLIBC
    if (constructor_count != 1 || initial_environ != envp || environ != envp ||
        initial_role != (empty ? '-' : role) || constructor_hash != hash) return 107;
    if (!same(getenv("OPTION"), empty ? NULL : first ? "x=y" : "p q=r") || getenv("ABSENT")) return 108;
#endif
    if (!receipt('M',role,count,hash)) return 109;
    return (empty ? 83 : 73) + !first;
}
