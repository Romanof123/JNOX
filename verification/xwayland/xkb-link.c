#include <stdio.h>
#include <string.h>
#include <X11/Xlib.h>
#include <X11/XKBlib.h>
#include <X11/extensions/XKBfile.h>
#include <X11/extensions/XKBrules.h>

int main(void)
{
    XkbRF_RulesPtr rules = XkbRF_Create(4, 2);
    if (!rules || rules->sz_rules != 4 || rules->sz_extra != 2)
        return 71;
    XkbRF_Free(rules, True);
    if (strcmp(XkbModIndexText(2, XkbXKBFile), "Control") != 0)
        return 72;
#ifdef JNOX_XKB_WITH_X11
    int major = XkbMajorVersion, minor = XkbMinorVersion;
    if (!XkbLibraryVersion(&major, &minor))
        return 73;
#endif
    return 0;
}
