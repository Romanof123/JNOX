/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_LIFECYCLE_BUS_H
#define JNOX_LIFECYCLE_BUS_H
struct virtio_device;
int jnox_lifecycle_probe_v1(void);
int jnox_lifecycle_open_v1(void);
int jnox_lifecycle_close_v1(void);
int jnox_lifecycle_remove_v1(void);
void jnox_lifecycle_stats_v1(unsigned long long out[12]);
int jnox_lifecycle_validate_v1(struct virtio_device *);
bool jnox_lifecycle_feature_v1(unsigned int bit);
void jnox_lifecycle_get_config_v1(struct virtio_device *, unsigned int, void *, unsigned int);
u8 jnox_lifecycle_get_status_v1(struct virtio_device *);
void jnox_lifecycle_set_status_v1(struct virtio_device *, u8);
void jnox_lifecycle_reset_v1(struct virtio_device *);
void jnox_lifecycle_del_vqs_v1(struct virtio_device *);
#endif
