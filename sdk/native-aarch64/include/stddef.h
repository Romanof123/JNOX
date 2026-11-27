#ifndef JNOX_STDDEF_H
#define JNOX_STDDEF_H
#define offsetof(type, member) __builtin_offsetof(type, member)
#define NULL ((void *)0)
typedef __SIZE_TYPE__ size_t;
#endif
