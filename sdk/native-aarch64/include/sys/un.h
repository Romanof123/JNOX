#ifndef JNOX_SYS_UN_H
#define JNOX_SYS_UN_H
#include <sys/socket.h>
struct sockaddr_un { sa_family_t sun_family; char sun_path[32]; };
#endif
