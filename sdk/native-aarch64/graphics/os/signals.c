#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stddef.h>
#include <stdint.h>
#include "jnox/native_abi.h"

_Static_assert(sizeof(struct sigaction) == 24 && offsetof(struct sigaction, sa_handler) == 0 &&
               offsetof(struct sigaction, sa_mask) == 8 && sizeof(sigset_t) == 8 &&
               offsetof(struct sigaction, sa_flags) == 16, "Picolibc AArch64 sigaction ABI");
_Static_assert(SIGHUP == 1 && SIGINT == 2 && SIGPIPE == 13 && SIGTERM == 15 && SIGUSR1 == 30,
               "Picolibc AArch64 signal numbers");
void jnox_graphics_errno(int);
extern void jnox_signal_restorer(void);

#define STRING_VALUE_(x) #x
#define STRING_VALUE(x) STRING_VALUE_(x)
__asm__(".pushsection .text.jnox_signal_restorer,\"ax\",%progbits\n"
        ".p2align 2\n.hidden jnox_signal_restorer\n.global jnox_signal_restorer\n"
        ".type jnox_signal_restorer,%function\njnox_signal_restorer:\n"
        "mov x8,#" STRING_VALUE(JNOX_CALL_SIGRETURN) "\n"
        "svc #" STRING_VALUE(JNOX_ABI_SVC) "\nbrk #0\n"
        ".size jnox_signal_restorer,.-jnox_signal_restorer\n.popsection");

int sigaction(int signal_number, const struct sigaction *restrict act,
              struct sigaction *restrict oldact)
{
    register long x0 __asm__("x0") = signal_number;
    register uintptr_t x1 __asm__("x1") = (uintptr_t)act;
    register uintptr_t x2 __asm__("x2") = (uintptr_t)oldact;
    register uintptr_t x3 __asm__("x3") = (uintptr_t)jnox_signal_restorer;
    register long x8 __asm__("x8") = JNOX_CALL_SIGACTION;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x1), "r"(x2), "r"(x3), "r"(x8)
                     : "memory", "cc");
    if (x0 == 0) return 0;
    jnox_graphics_errno((int)-x0);
    return -1;
}
