/* Standard CVT 1024x768 at nominal 60 Hz, progressive, normal blanking. */
#include <libxcvt/libxcvt.h>
#include <stdlib.h>

volatile struct libxcvt_mode_info jnox_xcvt_mode;
int main(void)
{
    struct libxcvt_mode_info *m = libxcvt_gen_mode_info(1024, 768, 60, false, false);
    if (!m) return 1;
    jnox_xcvt_mode = *m;
    free(m);
    if (jnox_xcvt_mode.hdisplay != 1024 || jnox_xcvt_mode.vdisplay != 768) return 2;
    if (jnox_xcvt_mode.dot_clock != 63500) return 3;
    if (jnox_xcvt_mode.hsync_start != 1072 || jnox_xcvt_mode.hsync_end != 1176 ||
        jnox_xcvt_mode.htotal != 1328) return 4;
    if (jnox_xcvt_mode.vsync_start != 771 || jnox_xcvt_mode.vsync_end != 775 ||
        jnox_xcvt_mode.vtotal != 798) return 5;
    if (jnox_xcvt_mode.mode_flags != (LIBXCVT_MODE_FLAG_HSYNC_NEGATIVE |
                                      LIBXCVT_MODE_FLAG_VSYNC_POSITIVE)) return 6;
    return 0;
}
