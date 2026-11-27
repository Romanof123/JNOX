/* Public libX11 entry point that can run before a display server is available. */
#include <X11/Xlib.h>
#include <X11/Xutil.h>

int main(void)
{
    int x = 0, y = 0;
    unsigned int width = 0, height = 0;
    int mask = XParseGeometry("=160x120-7+5", &x, &y, &width, &height);
    if (mask != (AllValues | XNegative) || x != -7 || y != 5 ||
        width != 160 || height != 120) return 11;
    mask = XParseGeometry("+3-2", &x, &y, &width, &height);
    if (mask != (XValue | YValue | YNegative) || x != 3 || y != -2 ||
        width != 160 || height != 120) return 12;
    if (XParseGeometry("10x", &x, &y, &width, &height) != NoValue) return 13;
    return 0;
}
