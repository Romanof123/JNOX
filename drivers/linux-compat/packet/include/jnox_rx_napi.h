/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_RX_NAPI_H
#define JNOX_RX_NAPI_H
struct net_device;
struct napi_struct;
struct virtqueue;
struct scatterlist;
void jnox_rx_napi_stats_v1(unsigned long long out[8]);
void jnox_rx_napi_event_v1(unsigned int mode, unsigned int value);
int jnox_rx_napi_prepare_v1(struct net_device *dev);
int jnox_rx_napi_poll_v1(struct napi_struct *napi, int budget);
int jnox_rx_napi_release_v1(struct net_device *dev);
int jnox_rx_queue_large_sg_v1(struct scatterlist *, unsigned int);
void jnox_rx_disable_cb_v1(struct virtqueue *);
bool jnox_rx_kick_prepare_v1(struct virtqueue *);
bool jnox_rx_notify_v1(struct virtqueue *);
#endif
