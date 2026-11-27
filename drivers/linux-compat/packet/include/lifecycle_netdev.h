/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_LIFECYCLE_NETDEV_H
#define JNOX_LIFECYCLE_NETDEV_H

struct net_device;
struct net_device *jnox_lifecycle_registered_v1(void);
/* Caller holds the native callback context. Each operation takes RTNL. */
int jnox_lifecycle_net_up_v1(struct net_device *dev);
int jnox_lifecycle_net_down_v1(struct net_device *dev);

enum jnox_lifecycle_netdev_stat {
    JN_ALLOC, JN_FREE, JN_REGISTER, JN_UNREGISTER,
    JN_RTNL_TAKES, JN_RTNL_DROPS, JN_RTNL_OWNER, JN_PRIVATE_SIZE,
    JN_DEVICE, JN_PRIVATE, JN_RX_QUEUE, JN_TX_QUEUE, JN_MAC_ADDRESS,
    JN_OPS, JN_OPEN_PC, JN_STOP_PC, JN_OPEN_CALLS, JN_UPS, JN_OPEN_RC,
    JN_STOP_CALLS, JN_DOWNS, JN_STOP_RC, JN_REGISTRY, JN_UP,
    JN_TX_COUNT, JN_RX_COUNT, JN_MAC_WRITES, JN_CARRIER_ON, JN_CARRIER_OFF,
    JN_XPS_CPU_MASK, JN_DQL_RESETS, JN_FAULT, JN_NETDEV_STAT_COUNT
};
extern volatile unsigned long long jnox_lifecycle_netdev_journal[JN_NETDEV_STAT_COUNT];
void jnox_lifecycle_netdev_stats_v1(unsigned long long out[JN_NETDEV_STAT_COUNT]);

#endif
