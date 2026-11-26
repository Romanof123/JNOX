/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_ORIGINAL_RX_H
#define JNOX_ORIGINAL_RX_H

int jnox_original_rx_init(void);
/* Call after all 32 posts succeed; read only after this returns zero. */
long jnox_rx_start_v1(void);
int jnox_original_rx_read(void *out, unsigned int capacity);

#endif
