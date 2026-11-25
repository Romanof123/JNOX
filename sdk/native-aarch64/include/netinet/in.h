#ifndef JNOX_NETINET_IN_H
#define JNOX_NETINET_IN_H
#include <sys/socket.h>

typedef unsigned short in_port_t;
typedef uint32_t in_addr_t;
struct in_addr { in_addr_t s_addr; };
struct sockaddr_in {
    sa_family_t sin_family;
    in_port_t sin_port;
    struct in_addr sin_addr;
    unsigned char sin_zero[8];
};
#define INADDR_ANY ((in_addr_t)0)
#define INADDR_LOOPBACK ((in_addr_t)0x7f000001)
#define IPPROTO_TCP 6
#define IPPROTO_UDP 17

/* The native-v1 ABI is little-endian AArch64. */
static inline in_port_t htons(in_port_t v) { return __builtin_bswap16(v); }
static inline in_port_t ntohs(in_port_t v) { return __builtin_bswap16(v); }
static inline in_addr_t htonl(in_addr_t v) { return __builtin_bswap32(v); }
static inline in_addr_t ntohl(in_addr_t v) { return __builtin_bswap32(v); }
#endif
