/* Board-independent tinystdio backend for one native EL0 graphics process. */
#include <errno.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include "jnox/native_abi.h"

static long native_io(unsigned op, int fd, void *buffer, size_t size)
{
    register long x0 __asm__("x0") = fd;
    register void *x1 __asm__("x1") = buffer;
    register size_t x2 __asm__("x2") = size;
    register unsigned long x8 __asm__("x8") = op;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x1), "r"(x2), "r"(x8) : "memory", "cc");
    if (x0 < 0) errno = (int)-x0;
    return x0;
}

int jnox_graphics_emit(const char *s, size_t size)
{
    return native_io(JNOX_CALL_WRITE, 1, (void *)s, size) == (long)size ? 0 : -1;
}

static int output(char c, FILE *stream)
{
    (void)stream;
    /* The native ABI exposes its process console as fd 1. */
    return native_io(JNOX_CALL_WRITE, 1, &c, 1) == 1 ? 0 : _FDEV_ERR;
}

static int input(FILE *stream)
{
    (void)stream;
    unsigned char c;
    long n = native_io(JNOX_CALL_READ, 0, &c, 1);
    return n == 1 ? c : n == 0 ? _FDEV_EOF : _FDEV_ERR;
}

static FILE console_out = FDEV_SETUP_STREAM(output, NULL, NULL, _FDEV_SETUP_WRITE);
static FILE console_in = FDEV_SETUP_STREAM(NULL, input, NULL, _FDEV_SETUP_READ);
FILE *const stdout = &console_out;
FILE *const stderr = &console_out;
FILE *const stdin = &console_in;

_Noreturn void _exit(int code)
{
    register long x0 __asm__("x0") = code;
    register long x8 __asm__("x8") = JNOX_CALL_EXIT;
    __asm__ volatile("svc #0x4a01" : "+r"(x0) : "r"(x8) : "cc", "memory");
    __builtin_trap();
}

_Noreturn void abort(void)
{
    static char message[] = "JNOX GRAPHICS ABORT\n";
    (void)native_io(JNOX_CALL_WRITE, 1, message, sizeof(message)-1);
    _exit(134);
}

extern void __libc_init_array(void);
extern int main(int argc, char **argv);

_Noreturn void jnox_graphics_start(int argc, char **argv)
{
    __libc_init_array();
    /* Picolibc pico-exit runs the fini array before calling native _exit. */
    exit(main(argc, argv));
}
