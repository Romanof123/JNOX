#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static volatile sig_atomic_t stage, repeats, broken;
static pid_t self;
static const struct sigaction read_only = {.sa_handler = SIG_IGN};

static void inner(int number)
{
    if (number != SIGUSR1 || stage != 1) broken = 1;
    stage = 2;
    __asm__ volatile("mov x9,#17\n movi v0.16b,#0x11" ::: "x9", "v0");
}

static void outer(int number)
{
    if (number != SIGHUP) broken = 1;
    if (++repeats == 1) {
        stage = 1;
        if (kill(self, SIGUSR1) || stage != 2) broken = 1;
        // Blocked by this handler. Both instances must coalesce into one delivery.
        if (kill(self, SIGHUP) || kill(self, SIGHUP) || repeats != 1) broken = 1;
        stage = 3;
    } else if (repeats == 2 && stage == 3) {
        stage = 4;
    } else {
        broken = 1;
    }
    __asm__ volatile("mov x9,#37\n movi v0.16b,#0x25" ::: "x9", "v0");
}

// Check the interrupted integer and SIMD values across a real self-signal
// SVC. Both callbacks explicitly modify those caller-saved registers.
static int signal_registers(void)
{
    register long x0 __asm__("x0") = self;
    register long x1 __asm__("x1") = SIGHUP;
    register long x2 __asm__("x2") = 0;
    register long x8 __asm__("x8") = 209;
    uint64_t lane, scratch;
    __asm__ volatile("mov x9,#735\n fmov d0,x9\n svc #0x4a01\n fmov %1,d0\n mov %2,x9"
                     : "+r"(x0), "=&r"(lane), "=&r"(scratch) : "r"(x1), "r"(x2), "r"(x8)
                     : "x9", "v0", "cc", "memory");
    return x0 == 0 && lane == 735 && scratch == 735;
}

int main(int argc, char **argv)
{
    self = getpid();
    const struct sigaction ignore = {.sa_handler = SIG_IGN};
    struct sigaction got = {0};
    if (argc == 2 && !strcmp(argv[1], "default")) {
        if (sigaction(SIGTERM, NULL, &got) || got.sa_handler != SIG_DFL) return 100;
        kill(self, SIGTERM);
        return 101; // default termination must leave through the kernel's normal exit path
    }
    if (sigaction(SIGPIPE, &ignore, &got) || got.sa_handler != SIG_DFL) return 102;
    if (kill(self, SIGPIPE)) return 103;
    struct sigaction act = {.sa_handler = inner};
    if (sigaction(SIGUSR1, &act, NULL)) return 104;
    act.sa_handler = outer;
    sigaddset(&act.sa_mask, SIGHUP);
    if (sigaction(SIGHUP, &act, NULL)) return 105;
    if (!signal_registers() || broken || stage != 4 || repeats != 2) return 106;
    if (sigaction(SIGHUP, NULL, &got) || got.sa_handler != outer || got.sa_mask != act.sa_mask) return 107;
    act.sa_flags = SA_SIGINFO;
    if (!sigaction(SIGHUP, &act, NULL) || errno != ENOTSUP) return 108;
    if (!sigaction(SIGSEGV, &ignore, NULL) || errno != ENOTSUP) return 109;
    if (!sigaction(SIGKILL, &ignore, NULL) || errno != EINVAL) return 110;
    if (!sigaction(SIGHUP, &ignore, (struct sigaction *)&read_only) || errno != EFAULT) return 112;
    act.sa_flags = 0;
    act.sa_handler = (void (*)(int))(uintptr_t)&stage;
    if (!sigaction(SIGHUP, &act, NULL) || errno != EINVAL) return 113;
    if (sigaction(SIGHUP, NULL, &got) || got.sa_handler != outer || got.sa_flags) return 111;
    return 86;
}
