// SPDX-License-Identifier: GPL-2.0-or-later
#ifndef JNOX_LIFECYCLE_PLATFORM_H
#define JNOX_LIFECYCLE_PLATFORM_H
int jnox_rx_lifecycle_ready_v1(void);
void jnox_rx_lifecycle_retire_v1(unsigned long epoch);
void jnox_lifecycle_run_work_v1(void);
unsigned int jnox_lifecycle_page_refs_v1(void);
#endif
