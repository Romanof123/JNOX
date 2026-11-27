/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef JNOX_LIFECYCLE_FAULTS_H
#define JNOX_LIFECYCLE_FAULTS_H
#ifdef JNOX_LIFECYCLE_FAULT_TEST

enum jnox_lifecycle_fault_case {
    JF_ALLOC = 1, JF_REGISTER, JF_CPU_DEAD, JF_XDP_MODEL, JF_CASES = JF_XDP_MODEL
};
enum jnox_lifecycle_fault_phase {
    JF_ARMED, JF_FIRED, JF_RETURNED, JF_CLEANED, JF_PHASES
};
enum jnox_lifecycle_fault_field {
    JF_ALLOCATED_DEV, JF_REGISTERED_DEV, JF_RTNL_OWNER, JF_DEV_FLAGS,
    JF_ONLINE_NODE, JF_DEAD_NODE, JF_REGISTERED_RXQ, JF_SHARED_MODEL,
    JF_QUEUE_TOKEN, JF_COMMON_LIVE, JF_DRIVER, JF_ALLOCS, JF_FREES,
    JF_REGISTRATIONS, JF_UNREGISTRATIONS, JF_DEV_STATE, JF_RX_VQ, JF_TX_VQ,
    JF_PRIVATE, JF_REG_STATE, JF_FIELDS
};
enum jnox_lifecycle_fault_event {
    JF_ARMED_ID, JF_FIRE_COUNT, JF_TOTAL_FIRED, JF_CANDIDATE, JF_CALLER_PC,
    JF_FIRED_ID, JF_VALID_PHASES, JF_ADAPTER_RESULT, JF_EVENTS
};
enum jnox_lifecycle_fault_resource {
    JF_POSTED_RX, JF_PAGE_REFS, JF_FIRST_POSTED_VA, JF_SKB_PENDING,
    JF_TOTAL_POSTED, JF_TOTAL_USED, JF_TOTAL_READY, JF_QUEUE_ERROR, JF_RESOURCES
};

extern volatile unsigned long long jnox_lifecycle_fault_state[JF_CASES][JF_PHASES][JF_FIELDS];
extern volatile unsigned long long jnox_lifecycle_fault_events[JF_CASES][JF_EVENTS];
extern volatile unsigned long long jnox_lifecycle_fault_resources[JF_CASES][JF_PHASES][JF_RESOURCES];
/* The private native service calls arm, then records RETURNED and CLEANED
 * after the upstream operation and its cleanup. Hook code records FIRED. */
int jnox_lifecycle_fault_arm_v1(unsigned int id);
int jnox_lifecycle_fault_snapshot_v1(unsigned int id, unsigned int phase);

/* Adapter-internal readers and one-shot decision. */
int jnox_lifecycle_fault_hit_v1(unsigned int id, const void *candidate, const void *caller);
void jnox_lifecycle_fault_read_netdev_v1(volatile unsigned long long out[JF_FIELDS]);
void jnox_lifecycle_fault_read_platform_v1(volatile unsigned long long out[JF_FIELDS]);

#endif
#endif
