// SPDX-License-Identifier: GPL-2.0-or-later
/* One native FIFO callback worker, one Ethernet interface and one queue pair. */
#include <jnox_virtnet_layout.h>
#include <jnox_packet_context.h>
#include <jnox_packet_tx_runtime.h>
#include <jnox_rx_napi.h>
#include <lifecycle_netdev.h>
#ifdef JNOX_LIFECYCLE_FAULT_TEST
#include <lifecycle_faults.h>
#endif
#include <linux/if_arp.h>
#include <linux/jiffies.h>
#include <linux/netlink.h>
#include <linux/rbtree_augmented.h>
#include <linux/rtnetlink.h>
#include <net/netdev_rx_queue.h>
#include <net/pkt_sched.h>

static union {
    struct net_device alignment;
    u8 bytes[offsetof(struct net_device, priv) + sizeof(struct virtnet_info)];
} storage;
static struct netdev_queue tx;
static struct netdev_rx_queue rx;
static struct netdev_hw_addr mac;
static struct net_device *allocated, *registered;
static struct task_struct *rtnl_owner;
static bool tx_frozen;
#ifdef CONFIG_XPS
static union {
    struct xps_map map;
    u8 bytes[XPS_MAP_SIZE(1)];
} cpu0_tx;
static union {
    struct xps_dev_maps maps;
    u8 bytes[sizeof(struct xps_dev_maps) + sizeof(struct xps_map *)];
} xps;
#endif

volatile unsigned long long jnox_lifecycle_netdev_journal[JN_NETDEV_STAT_COUNT];
#define journal jnox_lifecycle_netdev_journal
static_assert(JN_NETDEV_STAT_COUNT == 32);

void jnox_lifecycle_netdev_stats_v1(unsigned long long out[JN_NETDEV_STAT_COUNT])
{
    for (unsigned int i = 0; i < JN_NETDEV_STAT_COUNT; ++i)
        out[i] = READ_ONCE(journal[i]);
}

#ifdef JNOX_LIFECYCLE_FAULT_TEST
void jnox_lifecycle_fault_read_netdev_v1(volatile unsigned long long out[JF_FIELDS])
{
    out[JF_ALLOCATED_DEV] = (unsigned long)allocated;
    out[JF_REGISTERED_DEV] = (unsigned long)registered;
    out[JF_RTNL_OWNER] = (unsigned long)rtnl_owner;
    out[JF_DEV_FLAGS] = allocated ? allocated->flags : 0;
    out[JF_DEV_STATE] = allocated ? allocated->state : 0;
    out[JF_PRIVATE] = allocated ? (unsigned long)netdev_priv(allocated) : 0;
    out[JF_REG_STATE] = allocated ? allocated->reg_state : 0;
    out[JF_ALLOCS] = journal[JN_ALLOC];
    out[JF_FREES] = journal[JN_FREE];
    out[JF_REGISTRATIONS] = journal[JN_REGISTER];
    out[JF_UNREGISTRATIONS] = journal[JN_UNREGISTER];
}
#endif

static __noreturn void fault(unsigned int reason)
{
    journal[JN_FAULT] = reason;
    __builtin_trap();
}

static void require_device(const struct net_device *dev)
{
    if (!dev || dev != allocated)
        fault(1);
}

/* The timeout diagnostic takes a jiffies delta. This pinned kernel uses an
 * integral number of microseconds per tick and returns an unsigned int. */
unsigned int jiffies_to_usecs(const unsigned long ticks)
{
    static_assert(USEC_PER_SEC % HZ == 0);
    return (unsigned int)(ticks * (USEC_PER_SEC / HZ));
}

/* The three retained NL_SET_ERR_MSG_MOD sites precede original XDP errors.
 * Preserve the message locally and let the original return its own errno. */
char jnox_lifecycle_extack_message[NETLINK_MAX_FMTMSG_LEN];
unsigned long jnox_lifecycle_extack_count;

void do_trace_netlink_extack(const char *message)
{
    if (!message)
        fault(18);
    unsigned int i = 0;
    while (i + 1 < sizeof(jnox_lifecycle_extack_message) && message[i]) {
        jnox_lifecycle_extack_message[i] = message[i];
        ++i;
    }
    jnox_lifecycle_extack_message[i] = '\0';
    __atomic_add_fetch(&jnox_lifecycle_extack_count, 1UL, __ATOMIC_RELEASE);
}

/* VIRTIO_NET_S_ANNOUNCE requires the Linux L3 peer-notifier chain (ARP/ND).
 * That feature is outside this service. Reaching it must stop before the
 * original acknowledges the announcement to the device. */
void netdev_notify_peers(struct net_device *dev)
{
    require_device(dev);
    fault(17);
}

/* The only admitted callback task cannot sleep waiting for itself. An
 * attempted recursive blocking acquisition is a caller contract violation. */
int rtnl_trylock(void)
{
    struct task_struct *idle = NULL;
    struct task_struct *task = jnox_packet_current_v1();
    if (!__atomic_compare_exchange_n(&rtnl_owner, &idle, task, false,
                                     __ATOMIC_ACQUIRE, __ATOMIC_RELAXED))
        return 0;
    journal[JN_RTNL_OWNER] = (unsigned long)task;
    ++journal[JN_RTNL_TAKES];
    return 1;
}

void rtnl_lock(void)
{
    if (!rtnl_trylock())
        fault(2);
}

int rtnl_is_locked(void)
{
    return __atomic_load_n(&rtnl_owner, __ATOMIC_ACQUIRE) != NULL;
}

void rtnl_unlock(void)
{
    if (__atomic_load_n(&rtnl_owner, __ATOMIC_RELAXED) != jnox_packet_current_v1())
        fault(3);
    ++journal[JN_RTNL_DROPS];
    journal[JN_RTNL_OWNER] = 0;
    __atomic_store_n(&rtnl_owner, NULL, __ATOMIC_RELEASE);
}

/* The bounded Ethernet port supplies direct header operations. The native
 * worker consumes complete frames and has no neighbour/header cache. */
static int header_create(struct sk_buff *skb, struct net_device *dev, unsigned short type,
                         const void *dst, const void *src, unsigned int len)
{
    require_device(dev);
    if (!skb || skb_headroom(skb) < ETH_HLEN)
        return -ENOSPC;
    struct ethhdr *h = (struct ethhdr *)__skb_push(skb, ETH_HLEN);
    h->h_proto = htons(type == ETH_P_802_3 || type == ETH_P_802_2 ? len : type);
    ether_addr_copy(h->h_source, src ? src : dev->dev_addr);
    if (!dst)
        return -ETH_HLEN;
    ether_addr_copy(h->h_dest, dst);
    return ETH_HLEN;
}

static int header_parse(const struct sk_buff *skb, const struct net_device *dev, u8 *addr)
{
    require_device(dev);
    if (!skb || !addr || !skb_mac_header_was_set(skb) ||
        skb_mac_header(skb) + ETH_HLEN > skb_tail_pointer(skb))
        return 0;
    ether_addr_copy(addr, eth_hdr(skb)->h_source);
    return ETH_ALEN;
}

static __be16 header_protocol(const struct sk_buff *skb)
{
    if (!skb || !skb_mac_header_was_set(skb) ||
        skb_mac_header(skb) + ETH_HLEN > skb_tail_pointer(skb))
        return 0;
    return eth_hdr(skb)->h_proto;
}

static const struct header_ops ethernet_headers = {
    .create = header_create, .parse = header_parse, .parse_protocol = header_protocol,
};

void ether_setup(struct net_device *dev)
{
    require_device(dev);
    dev->header_ops = &ethernet_headers;
    dev->type = ARPHRD_ETHER;
    dev->hard_header_len = dev->min_header_len = ETH_HLEN;
    dev->mtu = dev->max_mtu = ETH_DATA_LEN;
    dev->min_mtu = ETH_MIN_MTU;
    dev->addr_len = ETH_ALEN;
    dev->tx_queue_len = DEFAULT_TX_QUEUE_LEN;
    dev->flags = IFF_BROADCAST | IFF_MULTICAST;
    dev->priv_flags |= IFF_TX_SKB_SHARING;
    eth_broadcast_addr(dev->broadcast);
}

static void init_addresses(struct net_device *dev)
{
    memset(&mac, 0, sizeof(mac));
    INIT_LIST_HEAD(&dev->uc.list);
    INIT_LIST_HEAD(&dev->mc.list);
    /* The permanent, writable primary address is also the only entry in
     * the Linux hardware address list and its red-black lookup tree. */
    struct list_head *head = &dev->dev_addrs.list;
    head->next = head->prev = &mac.list;
    mac.list.prev = mac.list.next = head;
    rb_set_parent_color(&mac.node, NULL, RB_BLACK);
    dev->dev_addrs.tree.rb_node = &mac.node;
    dev->dev_addrs.count = 1;
    mac.type = NETDEV_HW_ADDR_T_LAN;
    mac.refcount = 1;
    dev->dev_addr = mac.addr;
}

struct net_device *alloc_netdev_mqs(int bytes, const char *name, unsigned char name_type,
                                    void (*setup)(struct net_device *),
                                    unsigned int txqs, unsigned int rxqs)
{
    static const char pattern[] = "eth%d";
    if (allocated || bytes != sizeof(struct virtnet_info) || txqs != 1 || rxqs != 1 ||
        setup != ether_setup || !name)
        return NULL;
    for (unsigned int i = 0; i < sizeof(pattern); ++i)
        if (name[i] != pattern[i])
            return NULL;
#ifdef JNOX_LIFECYCLE_FAULT_TEST
    if (jnox_lifecycle_fault_hit_v1(JF_ALLOC, &storage.alignment, __builtin_return_address(0)))
        return NULL;
#endif
    memset(&storage, 0, sizeof(storage));
    memset(&tx, 0, sizeof(tx));
    memset(&rx, 0, sizeof(rx));
    struct net_device *dev = &storage.alignment;
    allocated = dev;
    dev->priv_len = bytes;
    memcpy(dev->name, "eth%d", sizeof("eth%d"));
    dev->name_assign_type = name_type;
    dev->_tx = &tx;
    dev->_rx = &rx;
    dev->num_tx_queues = dev->real_num_tx_queues = 1;
    dev->num_rx_queues = dev->real_num_rx_queues = 1;
    tx.dev = rx.dev = dev;
    tx.xmit_lock_owner = -1;
    spin_lock_init(&tx._xmit_lock);
#ifdef CONFIG_BQL
    tx.dql.min_limit = tx.dql.max_limit = ETH_FRAME_LEN;
    tx.dql.adj_limit = tx.dql.limit = ETH_FRAME_LEN;
#endif
    init_addresses(dev);
    setup(dev);
    if (jnox_rx_napi_prepare_v1(dev)) {
        allocated = NULL;
        return NULL;
    }
    ++journal[JN_ALLOC];
    journal[JN_PRIVATE_SIZE] = bytes;
    journal[JN_DEVICE] = (unsigned long)dev;
    journal[JN_PRIVATE] = (unsigned long)netdev_priv(dev);
    journal[JN_RX_QUEUE] = (unsigned long)&rx;
    journal[JN_TX_QUEUE] = (unsigned long)&tx;
    journal[JN_MAC_ADDRESS] = (unsigned long)dev->dev_addr;
    return dev;
}

struct net_device *alloc_etherdev_mqs(int bytes, unsigned int txqs, unsigned int rxqs)
{
    return alloc_netdev_mqs(bytes, "eth%d", NET_NAME_UNKNOWN, ether_setup, txqs, rxqs);
}

void dev_addr_mod(struct net_device *dev, unsigned int offset, const void *addr, size_t len)
{
    require_device(dev);
    if (!addr || offset > dev->addr_len || len > dev->addr_len - offset ||
        dev->dev_addr != mac.addr)
        fault(4);
    memcpy(mac.addr + offset, addr, len);
    memcpy(dev->dev_addr_shadow + offset, addr, len);
    ++journal[JN_MAC_WRITES];
}

int eth_validate_addr(struct net_device *dev)
{
    if (!dev || dev != allocated)
        return -ENODEV;
    return is_valid_ether_addr(dev->dev_addr) ? 0 : -EADDRNOTAVAIL;
}

int eth_prepare_mac_addr_change(struct net_device *dev, void *p)
{
    const struct sockaddr *address = p;
    if (!dev || dev != allocated || !address)
        return -EINVAL;
    if (!(dev->priv_flags & IFF_LIVE_ADDR_CHANGE) && netif_running(dev))
        return -EBUSY;
    return is_valid_ether_addr((const u8 *)address->sa_data) ? 0 : -EADDRNOTAVAIL;
}

void eth_commit_mac_addr_change(struct net_device *dev, void *p)
{
    const struct sockaddr *address = p;
    if (eth_prepare_mac_addr_change(dev, p))
        fault(5);
    eth_hw_addr_set(dev, (const u8 *)address->sa_data);
}

int netif_set_real_num_tx_queues(struct net_device *dev, unsigned int count)
{
    if (!dev || dev != allocated || !count || count > dev->num_tx_queues)
        return -EINVAL;
    dev->real_num_tx_queues = count;
    journal[JN_TX_COUNT] = count;
    return 0;
}

int netif_set_real_num_rx_queues(struct net_device *dev, unsigned int count)
{
    if (!dev || dev != allocated || !count || count > dev->num_rx_queues)
        return -EINVAL;
    dev->real_num_rx_queues = count;
    journal[JN_RX_COUNT] = count;
    return 0;
}

void netif_queue_set_napi(struct net_device *dev, unsigned int index,
                          enum netdev_queue_type type, struct napi_struct *napi)
{
    require_device(dev);
    if (index || (napi && napi->dev != dev))
        fault(6);
    struct napi_struct **entry;
    if (type == NETDEV_QUEUE_TYPE_RX)
        entry = &rx.napi;
    else if (type == NETDEV_QUEUE_TYPE_TX)
        entry = &tx.napi;
    else
        fault(7);
    if (napi && *entry && *entry != napi)
        fault(8);
    WRITE_ONCE(*entry, napi);
}

#ifdef CONFIG_XPS
int __netif_set_xps_queue(struct net_device *dev, const unsigned long *mask, u16 index,
                          enum xps_map_type type)
{
    if (!dev || dev != allocated || !mask || index || type != XPS_CPUS || *mask > 1UL)
        return -EINVAL;
    xps.maps.nr_ids = 1;
    xps.maps.num_tc = 1;
    cpu0_tx.map.queues[0] = 0;
    cpu0_tx.map.len = cpu0_tx.map.alloc_len = 1;
    xps.maps.attr_map[0] = &cpu0_tx.map;
    WRITE_ONCE(dev->xps_maps[XPS_CPUS], *mask ? &xps.maps : NULL);
    journal[JN_XPS_CPU_MASK] = *mask;
    return 0;
}
#endif

#ifdef CONFIG_BQL
void dql_reset(struct dql *d)
{
    if (!allocated || d != &tx.dql || d->min_limit != ETH_FRAME_LEN ||
        d->max_limit != ETH_FRAME_LEN)
        fault(9);
    memset(d, 0, sizeof(*d));
    d->min_limit = d->max_limit = ETH_FRAME_LEN;
    d->adj_limit = d->limit = ETH_FRAME_LEN;
    ++journal[JN_DQL_RESETS];
}
#endif

void netif_carrier_on(struct net_device *dev)
{
    require_device(dev);
    if (test_and_clear_bit(__LINK_STATE_NOCARRIER, &dev->state)) {
        atomic_inc(&dev->carrier_up_count);
        ++journal[JN_CARRIER_ON];
    }
}

void netif_carrier_off(struct net_device *dev)
{
    require_device(dev);
    if (!test_and_set_bit(__LINK_STATE_NOCARRIER, &dev->state)) {
        atomic_inc(&dev->carrier_down_count);
        ++journal[JN_CARRIER_OFF];
    }
}

void netif_tx_stop_all_queues(struct net_device *dev)
{
    require_device(dev);
    netif_tx_stop_queue(&tx);
}

void netif_tx_wake_queue(struct netdev_queue *q)
{
    if (!allocated || q != &tx)
        fault(16);
    if (test_and_clear_bit(__QUEUE_STATE_DRV_XOFF, &q->state))
        netif_schedule_queue(q);
}

void netif_device_detach(struct net_device *dev)
{
    require_device(dev);
    if (test_and_clear_bit(__LINK_STATE_PRESENT, &dev->state) && netif_running(dev))
        netif_tx_stop_all_queues(dev);
}

void netif_device_attach(struct net_device *dev)
{
    require_device(dev);
    if (!test_and_set_bit(__LINK_STATE_PRESENT, &dev->state) && netif_running(dev))
        netif_tx_wake_queue(&tx);
}

/* Freeze the one queue under its actual xmit lock. The freeze bit persists
 * between calls so the detach path can manipulate the stopped bit. */
void netif_tx_lock(struct net_device *dev)
{
    require_device(dev);
    if (tx_frozen)
        fault(10);
    __netif_tx_lock(&tx, 0);
    set_bit(__QUEUE_STATE_FROZEN, &tx.state);
    tx_frozen = true;
    __netif_tx_unlock(&tx);
}

void netif_tx_unlock(struct net_device *dev)
{
    require_device(dev);
    if (!tx_frozen)
        fault(11);
    __netif_tx_lock(&tx, 0);
    clear_bit(__QUEUE_STATE_FROZEN, &tx.state);
    tx_frozen = false;
    __netif_tx_unlock(&tx);
    netif_schedule_queue(&tx);
}

struct net_device *jnox_lifecycle_registered_v1(void)
{
    return __atomic_load_n(&registered, __ATOMIC_ACQUIRE);
}

int register_netdevice(struct net_device *dev)
{
    if (__atomic_load_n(&rtnl_owner, __ATOMIC_RELAXED) != jnox_packet_current_v1())
        return -EPERM;
    if (!dev || dev != allocated || !dev->dev.parent)
        return -ENODEV;
    if (registered || dev->reg_state != NETREG_UNINITIALIZED)
        return -EBUSY;
    const struct net_device_ops *ops = dev->netdev_ops;
    if (!ops || !ops->ndo_open || !ops->ndo_stop || !ops->ndo_start_xmit ||
        !dev->real_num_rx_queues || !dev->real_num_tx_queues)
        return -EINVAL;
#ifdef JNOX_LIFECYCLE_FAULT_TEST
    if (jnox_lifecycle_fault_hit_v1(JF_REGISTER, dev, __builtin_return_address(0)))
        return -ENOMEM;
#endif
    if (ops->ndo_init) {
        int rc = ops->ndo_init(dev);
        if (rc)
            return rc < 0 ? rc : -EIO;
    }
    memcpy(dev->name, "eth0", sizeof("eth0"));
    dev->name_assign_type = NET_NAME_ENUM;
    if (dev->addr_assign_type == NET_ADDR_PERM)
        ether_addr_copy(dev->perm_addr, dev->dev_addr);
    dev->ifindex = 1;
    dev->reg_state = NETREG_REGISTERED;
    set_bit(__LINK_STATE_PRESENT, &dev->state);
    __atomic_store_n(&registered, dev, __ATOMIC_RELEASE);
    ++journal[JN_REGISTER];
    journal[JN_REGISTRY] = (unsigned long)dev;
    journal[JN_OPS] = (unsigned long)ops;
    journal[JN_OPEN_PC] = (unsigned long)ops->ndo_open;
    journal[JN_STOP_PC] = (unsigned long)ops->ndo_stop;
    return 0;
}

static int open_locked(struct net_device *dev)
{
    if (!dev || dev != registered || !netif_device_present(dev))
        return -ENODEV;
    if (dev->flags & IFF_UP)
        return -EBUSY;
    const struct net_device_ops *ops = dev->netdev_ops;
    /* netif_running() is visible to the driver during open. Administrative
     * IFF_UP becomes visible only once the original method returns success. */
    set_bit(__LINK_STATE_START, &dev->state);
    int rc = ops->ndo_validate_addr ? ops->ndo_validate_addr(dev) : 0;
    if (!rc) {
        ++journal[JN_OPEN_CALLS];
        rc = ops->ndo_open(dev);
    }
    journal[JN_OPEN_RC] = (long)rc;
    if (rc) {
        clear_bit(__LINK_STATE_START, &dev->state);
        return rc;
    }
    dev->flags |= IFF_UP;
    journal[JN_UP] = 1;
    ++journal[JN_UPS];
    return 0;
}

int jnox_lifecycle_net_up_v1(struct net_device *dev)
{
    if (!rtnl_trylock())
        return -EBUSY;
    int rc = open_locked(dev);
    rtnl_unlock();
    return rc;
}

static int close_locked(struct net_device *dev)
{
    if (!dev || dev != registered)
        return -ENODEV;
    if (!(dev->flags & IFF_UP))
        return -EINVAL;
    clear_bit(__LINK_STATE_START, &dev->state);
    smp_mb__after_atomic();
    ++journal[JN_STOP_CALLS];
    int rc = dev->netdev_ops->ndo_stop(dev);
    journal[JN_STOP_RC] = (long)rc;
    if (rc) {
        set_bit(__LINK_STATE_START, &dev->state);
        return rc;
    }
    dev->flags &= ~IFF_UP;
    journal[JN_UP] = 0;
    ++journal[JN_DOWNS];
    return 0;
}

int jnox_lifecycle_net_down_v1(struct net_device *dev)
{
    if (!rtnl_trylock())
        return -EBUSY;
    int rc = close_locked(dev);
    rtnl_unlock();
    return rc;
}

void unregister_netdev(struct net_device *dev)
{
    rtnl_lock();
    if (!dev || dev != registered || dev->reg_state != NETREG_REGISTERED)
        fault(12);
    if ((dev->flags & IFF_UP) && close_locked(dev))
        fault(13);
    dev->reg_state = NETREG_UNREGISTERING;
    __atomic_store_n(&registered, NULL, __ATOMIC_RELEASE);
    journal[JN_REGISTRY] = 0;
    clear_bit(__LINK_STATE_PRESENT, &dev->state);
    if (dev->netdev_ops->ndo_uninit)
        dev->netdev_ops->ndo_uninit(dev);
    dev->reg_state = NETREG_UNREGISTERED;
    ++journal[JN_UNREGISTER];
    rtnl_unlock();
}

void free_netdev(struct net_device *dev)
{
    require_device(dev);
    if (registered || tx_frozen || (dev->flags & IFF_UP) ||
        (dev->reg_state != NETREG_UNINITIALIZED && dev->reg_state != NETREG_UNREGISTERED))
        fault(14);
    if (jnox_rx_napi_release_v1(dev))
        fault(15);
    /* A wake belongs to this queue generation. Consume the outstanding
     * scheduler notification before returning the static storage. */
    jnox_packet_tx_take_wake_v1(&tx);
    dev->reg_state = NETREG_RELEASED;
    allocated = NULL;
    ++journal[JN_FREE];
}
