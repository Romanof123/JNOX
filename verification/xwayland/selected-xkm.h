#ifndef JNOX_SELECTED_XKM_H
#define JNOX_SELECTED_XKM_H
#include <stddef.h>
#include <X11/Xdefs.h>
extern size_t jnox_selected_xkb_size(void);
extern const char *jnox_selected_xkb_identify(const void *, size_t);
extern Bool jnox_selected_xkb_wire(const void *, size_t);
#endif
