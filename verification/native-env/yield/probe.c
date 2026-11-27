#include <stddef.h>
#ifdef CHECK_PICOLIBC
#include <stdlib.h>
extern char **environ;
extern int jnox_graphics_emit(const char *, size_t);
#else
#include <unistd.h>
#endif

static int equal(const char *a, const char *b) {
    if (!a || !b) return a == b;
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

static int check(char **env, int a) {
    if (!env || !equal(env[0], a ? "WAYLAND_DISPLAY=/wayland-0" : "WAYLAND_DISPLAY=/wayland-1") ||
        !equal(env[1], a ? "A_ONLY=one" : "B_ONLY=two") || env[2]) return 0;
#ifdef CHECK_PICOLIBC
    if (environ != env || !equal(getenv("WAYLAND_DISPLAY"), a ? "/wayland-0" : "/wayland-1") ||
        !equal(getenv(a ? "A_ONLY" : "B_ONLY"),a ? "one" : "two") ||
        getenv(a ? "B_ONLY" : "A_ONLY") || getenv("JNOX_ENV_HOST_CANARY")) return 0;
#endif
    return 1;
}

__attribute__((noinline)) static long native_yield(void) {
    register unsigned long x8 __asm__("x8") = 8;
    register long x0 __asm__("x0") = 0;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x8) : "memory","cc");
    return x0;
}

static int emit(char role, char phase) {
    char line[]="ENV YIELD A 0\n";
    line[10]=role; line[12]=phase;
#ifdef CHECK_PICOLIBC
    return jnox_graphics_emit(line,sizeof(line)-1)==0;
#else
    return write(1,line,sizeof(line)-1)==sizeof(line)-1;
#endif
}

int main(int argc, char **argv, char **env) {
    if (argc != 2 || !argv || argv[2] || env != argv+3) return 100;
    int a = equal(argv[1],"A");
    if (!a && !equal(argv[1],"B")) return 101;
    if (!check(env,a) || !emit(a?'A':'B','0')) return 102;
    if (native_yield() != 0) return 103;
    if (!check(env,a) || !emit(a?'A':'B','1')) return 104;
    return a ? 91 : 92;
}
