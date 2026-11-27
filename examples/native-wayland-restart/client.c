/* Reuse the stock client protocol state machine for both connections. */
#include <sys/un.h>
#include "../native-wayland/pixels.h"
static unsigned palette_generation;
#define paint(p, t, count) paint(p, t, (count) + palette_generation)
static int saw_eof;
static ssize_t observe_recv(int fd, struct msghdr *msg, int flags)
{
    ssize_t count = recvmsg(fd, msg, flags);
    if (count == 0) saw_eof = 1;
    return count;
}
#define recvmsg observe_recv
#define main ordinary_wayland_client_main
#include "../native-wayland/client.c"
#undef main
#undef recvmsg

volatile uint64_t restart_client_sessions[2][32];
volatile uint64_t restart_client_epoch;
volatile uint64_t restart_client_eof[2];

static void snapshot(unsigned epoch)
{
    for (unsigned i = 0; i < 32; ++i) restart_client_sessions[epoch][i] = W[i];
}

static int release_connection(void)
{
    while (queued_count) REQUIRE(jnox_shm_close(take_fd()) == 0);
    REQUIRE(jnox_shm_unmap((void *)mapping) == 0 && close(channel) == 0);
    mapping = 0;
    for (unsigned i = 0; i < 4; ++i) globals[i] = 0;
    for (unsigned i = 0; i < 32; ++i) W[i] = 0;
    start_done = caps = keyboard_requested = surface_created = toplevel_configured = 0;
    busy = frame_active = frame_done = damage_pending = die_after_commit = closing = 0;
    saw_eof = 0;
    return 1;
}

int main(int argc, char **argv)
{
    static const char arg[] = "reconnect";
    if (argc != 2 || !argv[1]) return 99;
    for (unsigned i = 0; i < sizeof arg; ++i) if (argv[1][i] != arg[i]) return 99;
    for (unsigned epoch = 0; epoch < 2; ++epoch) {
        restart_client_epoch = epoch + 1;
        W[ROLE] = 1;
        /* Change the painted workload while retaining the Pulse app_id and title. */
        palette_generation = epoch;
        int clean = run();
        if (clean || !saw_eof || W[FRAMES] < 2 || !W[KEY_RELEASES]) {
            snapshot(epoch);
            return clean && W[CLOSE_SYNC_DONE] ? 71 : 102;
        }
        /* The stock event loop records a failed read on orderly peer EOF. */
        W[FAIL_LINE] = W[FAIL_ERRNO] = 0;
        snapshot(epoch);
        restart_client_eof[epoch] = 1;
        if (!release_connection()) return 103;
    }
    return 72;
}
