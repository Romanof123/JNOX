#include <wayland-client.h>

/* Launch contract: fd 3 is an already-connected Wayland stream. */
int main(void)
{
    struct wl_display *display = wl_display_connect_to_fd(3);
    if (!display)
        return 1;
    int status = wl_display_roundtrip(display);
    wl_display_disconnect(display);
    return status < 0 ? 2 : 0;
}
