//! Native joinable-thread C bindings. A nonzero return is a positive errno.

use core::ffi::c_void;
pub use crate::Timespec;

pub const CLOCK_MONOTONIC: i32 = 1;
pub const SCHED_FIFO: i32 = 1;
pub const SCHED_RR: i32 = 2;
pub const PTHREAD_INHERIT_SCHED: i32 = 0;
pub const PTHREAD_EXPLICIT_SCHED: i32 = 1;
pub const PTHREAD_PRIO_NONE: i32 = 0;
pub const PTHREAD_PRIO_INHERIT: i32 = 1;
pub const PTHREAD_PRIO_PROTECT: i32 = 2;
pub const ETIMEDOUT: i32 = 110;

#[repr(C)]
pub struct SchedParam {
    pub sched_priority: i32,
}

#[repr(C)]
pub struct PthreadAttr {
    opaque: u64,
}

/// Storage for a process-private native mutex. Use a stable address for the
/// whole initialized lifetime (e.g. static MaybeUninit<PthreadMutex>).
#[repr(C, align(8))]
pub struct PthreadMutex {
    opaque: u64,
}

#[repr(C)]
pub struct PthreadMutexAttr {
    opaque: u64,
}

extern "C" {
    pub fn pthread_attr_init(attr: *mut PthreadAttr) -> i32;
    pub fn pthread_attr_destroy(attr: *mut PthreadAttr) -> i32;
    pub fn pthread_attr_setschedpolicy(attr: *mut PthreadAttr, policy: i32) -> i32;
    pub fn pthread_attr_setschedparam(attr: *mut PthreadAttr, param: *const SchedParam) -> i32;
    pub fn pthread_attr_setinheritsched(attr: *mut PthreadAttr, inherit: i32) -> i32;
    pub fn pthread_mutexattr_init(attr: *mut PthreadMutexAttr) -> i32;
    pub fn pthread_mutexattr_destroy(attr: *mut PthreadMutexAttr) -> i32;
    pub fn pthread_mutexattr_setprotocol(attr: *mut PthreadMutexAttr, protocol: i32) -> i32;
    pub fn pthread_mutexattr_setprioceiling(attr: *mut PthreadMutexAttr, ceiling: i32) -> i32;
    pub fn jnox_mutexattr_setusers(attr: *mut PthreadMutexAttr, mask: u32) -> i32;
    pub fn pthread_mutex_clocklock(mutex: *mut PthreadMutex, clock: i32, absolute: *const Timespec) -> i32;

    /// Initialize writable storage with null or initialized mutex attributes. Serialize against all
    /// other uses; the storage may be uninitialized on entry.
    pub fn pthread_mutex_init(mutex: *mut PthreadMutex, attr: *const PthreadMutexAttr) -> i32;
    /// Destroy an initialized mutex; exclude concurrent use during destruction.
    pub fn pthread_mutex_destroy(mutex: *mut PthreadMutex) -> i32;
    /// Acquire at the original address, blocking until ownership if necessary.
    pub fn pthread_mutex_lock(mutex: *mut PthreadMutex) -> i32;
    /// Acquire immediately or return a positive errno.
    pub fn pthread_mutex_trylock(mutex: *mut PthreadMutex) -> i32;
    /// Release a mutex owned by the calling thread.
    pub fn pthread_mutex_unlock(mutex: *mut PthreadMutex) -> i32;

    /// Start a worker with default (null) or initialized scheduling attributes.
    ///
    /// # Safety
    /// Provide writable handle storage. The routine and everything it accesses
    /// through its argument must remain valid until it finishes. Synchronize
    /// shared mutable access and never unwind across the C entry point.
    pub fn pthread_create(
        thread: *mut u64,
        attr: *const PthreadAttr,
        start: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
        arg: *mut c_void,
    ) -> i32;

    /// Collect a worker's opaque result or discard it with a null output.
    ///
    /// # Safety
    /// A non-null output must be writable pointer storage. Check success and
    /// establish the returned pointer's validity before dereferencing it.
    pub fn pthread_join(thread: u64, result: *mut *mut c_void) -> i32;

    /// End the calling thread in a threaded package.
    ///
    /// # Safety
    /// Stack destructors do not run; preserve any resources needed by other
    /// threads. A pointer to worker stack data cannot be used after exit.
    pub fn pthread_exit(value: *mut c_void) -> !;
}
