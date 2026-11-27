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
volatile uint64_t restart_pointer_sessions[2][11];
volatile uint64_t restart_pointer_button_held[2];
volatile int64_t restart_tablet_denials[2][2];
volatile uint64_t restart_client_epoch;
volatile uint64_t restart_client_eof[2];

static void snapshot(unsigned epoch)
{
    for (unsigned i = 0; i < 32; ++i) restart_client_sessions[epoch][i] = W[i];
    for (unsigned i = 0; i < 11; ++i) restart_pointer_sessions[epoch][i] = P[i];
    restart_pointer_button_held[epoch] = button_down;
}

static int release_connection(void)
{
    while (queued_count) REQUIRE(jnox_shm_close(take_fd()) == 0);
    REQUIRE(jnox_shm_unmap((void *)mapping) == 0 && close(channel) == 0);
    mapping = 0;
    for (unsigned i = 0; i < 4; ++i) globals[i] = 0;
    for (unsigned i = 0; i < 32; ++i) W[i] = 0;
    for (unsigned i = 0; i < 11; ++i) P[i] = 0;
    pointer_requested = pointer_focused = button_down = 0;
    start_done = caps = keyboard_requested = surface_created = toplevel_configured = 0;
    busy = frame_active = frame_done = damage_pending = die_after_commit = closing = 0;
    saw_eof = 0;
    return 1;
}

static int same_arg(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return *a == *b;
}

int main(int argc, char **argv)
{
    if (argc != 2 || !argv[1]) return 99;
    pointer_profile = same_arg(argv[1], "reconnect-pointer");
    if (!pointer_profile && !same_arg(argv[1], "reconnect")) return 99;
    for (unsigned epoch = 0; epoch < 2; ++epoch) {
        restart_client_epoch = epoch + 1;
        W[ROLE] = 1;
        if (pointer_profile) {
            for (unsigned command = 0; command < 2; ++command) {
                restart_tablet_denials[epoch][command] = jnox_display_call(197, command, 0);
                if (restart_tablet_denials[epoch][command] != -EPERM) return 104;
            }
        }
        /* Change the painted workload while retaining the Pulse app_id and title. */
        palette_generation = pointer_profile ? 2 * epoch : epoch;
        int clean = run();
        int has_pointer = pointer_profile && (P[P_CAPS] & CAP_POINTER);
        int input_seen = has_pointer ? P[P_PRESSES] != 0 : W[KEY_RELEASES] != 0;
        if (clean || !saw_eof || W[FRAMES] < 2 || !input_seen) {
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
