#ifndef JNOX_GRAPHICS_FCNTL_H
#define JNOX_GRAPHICS_FCNTL_H
#include_next <fcntl.h>

/* Commands for the graphics profile's resizable shared backing descriptors. */
#define F_ADD_SEALS 1033
#define F_GET_SEALS 1034
#define F_SEAL_SEAL 1
#define F_SEAL_SHRINK 2
#define F_SEAL_GROW 4
#endif
