#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <errno.h>
#include <setjmp.h>
#include <signal.h>
#include <stdio.h>

typedef void (*OsSigHandlerPtr)(int);
static int ParentProcess, RunFromSmartParent, getppid_calls;
static int actions, fail_on, errors;
static OsSigHandlerPtr disposition;
static jmp_buf failed;

static void GiveUp(int sig) { (void)sig; }
static void AutoResetServer(int sig) { (void)sig; }
static void report_error(const char *s) { (void)s; ++errors; }
static int parent_pid(void) { ++getppid_calls; return 42; }
static _Noreturn void FatalError(const char *format, ...) { (void)format; longjmp(failed, 1); }

static int action(int sig, const struct sigaction *next, struct sigaction *previous)
{
    assert(next->sa_handler != SIG_ERR);
    assert((sigismember(&next->sa_mask, sig) == 1) == (next->sa_handler != SIG_IGN));
    if (++actions == fail_on) {
        /* An unsuccessful sigaction leaves the output unspecified. */
        previous->sa_handler = GiveUp;
        errno = EINVAL;
        return -1;
    }
    previous->sa_handler = disposition;
    disposition = next->sa_handler;
    return 0;
}

#define sigaction(...) action(__VA_ARGS__)
#define perror report_error
#define getppid parent_pid
#include "server.inc"

static void reset(OsSigHandlerPtr previous, int failure)
{
    disposition = previous;
    fail_on = failure;
    actions = errors = getppid_calls = RunFromSmartParent = ParentProcess = 0;
}

static void expect_fatal(void (*operation)(void), int call)
{
    reset(SIG_DFL, call);
    if (setjmp(failed) == 0) {
        operation();
        assert(0 && "startup accepted a failed signal operation");
    }
    assert(actions == call && errors == 1 && !RunFromSmartParent && getppid_calls == 0);
}

int main(void)
{
    reset(SIG_DFL, 1);
    assert(OsSignal(SIGUSR1, SIG_IGN) == SIG_ERR && errors == 1);
    expect_fatal(InitParentProcess, 1);
    expect_fatal(InitParentProcess, 2);
    const OsSigHandlerPtr inherited[] = {SIG_DFL, SIG_IGN, GiveUp};
    for (unsigned int i = 0; i < sizeof(inherited) / sizeof(inherited[0]); ++i) {
        reset(inherited[i], 0);
        InitParentProcess();
        assert(disposition == inherited[i] && actions == 2 && errors == 0);
        assert(RunFromSmartParent == (inherited[i] == SIG_IGN) && ParentProcess == 42 && getppid_calls == 1);
    }
    for (int call = 1; call <= 4; ++call) expect_fatal(install_startup, call);
    reset(SIG_DFL, 0);
    install_startup();
    assert(actions == 4 && errors == 0);
    puts("Xwayland signal setup: old disposition, both parent failures, four startup failures and success passed");
}
