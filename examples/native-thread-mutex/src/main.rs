#![no_std]
#![no_main]

use core::ffi::c_void;
use core::panic::PanicInfo;
use core::ptr::{addr_of, addr_of_mut, null_mut, read_volatile};
use jnox_native::pthread::{
    pthread_mutex_lock, pthread_mutex_trylock, pthread_mutex_unlock, PthreadMutex,
};

#[repr(C)]
struct Aggregate {
    count: u64,
    sum: u64,
    sum_of_squares: u64,
}

extern "C" {
    static sensor_samples: [u32; 256];
    static mut sensor_mutex: PthreadMutex;
    static mut worker_try: i32;
    static mut worker_lock: i32;
    static mut worker_errno_after: i32;
    static mut nonowner_error: i32;
    static controls_mode: i32;
    fn announce_attempt();
    fn __jnox_errno_location() -> *mut i32;
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    jnox_native::exit(120)
}

// C creates this worker while holding sensor_mutex, with a process-lifetime
// Aggregate as the argument. The worker takes a reference only after locking.
#[no_mangle]
pub unsafe extern "C" fn sensor_worker(arg: *mut c_void) -> *mut c_void {
    let mutex = addr_of_mut!(sensor_mutex);
    let errno = __jnox_errno_location();
    *errno = 38;
    worker_try = pthread_mutex_trylock(mutex);
    if controls_mode != 0 {
        nonowner_error = pthread_mutex_unlock(mutex);
    }
    announce_attempt();
    if worker_try != 16 || (controls_mode != 0 && nonowner_error != 1) {
        return null_mut();
    }
    worker_lock = pthread_mutex_lock(mutex);
    if worker_lock != 0 {
        return null_mut();
    }
    {
        let total = &mut *arg.cast::<Aggregate>();
        for index in 128..256 {
            let sample = u64::from(read_volatile(addr_of!(sensor_samples[index])));
            total.count += 1;
            total.sum += sample;
            total.sum_of_squares += sample * sample;
        }
    }
    if pthread_mutex_unlock(mutex) != 0 {
        return null_mut();
    }
    worker_errno_after = *errno;
    arg
}
