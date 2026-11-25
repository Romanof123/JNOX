//! Native joinable-thread C bindings. A nonzero return is a positive errno.

use core::ffi::c_void;

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
    /// Initialize writable storage with null attributes. Serialize against all
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

    /// Start a worker with default attributes (null).
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
