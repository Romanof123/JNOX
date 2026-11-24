//! Linux host calibration for #184. Build with rustc and use the paired Python runner.
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
compile_error!("the host clock ABI of this fixture requires 64-bit Linux");

use std::fs;
use std::hint::black_box;
use std::os::raw::{c_int, c_long};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

const MONOTONIC: c_int = 1;
const THREAD_CPU: c_int = 3;

#[repr(C)]
#[derive(Default)]
struct Timespec {
    sec: c_long,
    nsec: c_long,
}

unsafe extern "C" {
    fn clock_gettime(id: c_int, time: *mut Timespec) -> c_int;
    fn clock_getres(id: c_int, time: *mut Timespec) -> c_int;
    fn sched_getscheduler(pid: c_int) -> c_int;
    fn getpriority(which: c_int, who: u32) -> c_int;
}

fn clock_ns(id: c_int, resolution: bool) -> u64 {
    let mut t = Timespec::default();
    let rc = unsafe {
        if resolution { clock_getres(id, &mut t) } else { clock_gettime(id, &mut t) }
    };
    assert_eq!(rc, 0, "clock access failed");
    assert!(t.sec >= 0 && (0..1_000_000_000).contains(&t.nsec));
    t.sec as u64 * 1_000_000_000 + t.nsec as u64
}

fn status_value<'a>(status: &'a str, key: &str) -> &'a str {
    status.lines().find_map(|line| line.split_once(':').filter(|(k, _)| *k == key))
        .expect("thread status field").1.trim()
}

fn identity(role: &str, cpu: u32) {
    let status = fs::read_to_string("/proc/thread-self/status").expect("thread status");
    let tid = status_value(&status, "Pid");
    let allowed = status_value(&status, "Cpus_allowed_list");
    let policy = unsafe { sched_getscheduler(0) };
    let nice = unsafe { getpriority(0, 0) };
    println!("thread {role} {tid} {allowed} {policy} {nice}");
    assert_eq!(allowed, cpu.to_string());
    assert_eq!((policy, nice), (0, 0), "requires SCHED_OTHER, nice 0");
}

fn switches() -> [u64; 2] {
    let status = fs::read_to_string("/proc/thread-self/status").expect("thread status");
    ["voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"].map(|key| {
        status_value(&status, key).parse().expect("switch count")
    })
}

struct Sample {
    w0: u64,
    c0: u64,
    sched: Option<[u64; 3]>,
    c1: u64,
    w1: u64,
}

impl Sample {
    fn take() -> Self {
        let w0 = clock_ns(MONOTONIC, false);
        let c0 = clock_ns(THREAD_CPU, false);
        let raw = fs::read_to_string("/proc/thread-self/schedstat");
        let c1 = clock_ns(THREAD_CPU, false);
        let w1 = clock_ns(MONOTONIC, false);
        let sched = raw.ok().and_then(|text| {
            let mut words = text.split_whitespace();
            let fields = [words.next()?.parse().ok()?, words.next()?.parse().ok()?,
                words.next()?.parse().ok()?];
            words.next().is_none().then_some(fields)
        });
        Self { w0, c0, sched, c1, w1 }
    }

    fn emit(&self, name: &str, side: &str) {
        let sched = match self.sched {
            Some([runtime, wait, slices]) => format!("{runtime} {wait} {slices}"),
            None => "NA NA NA".to_owned(),
        };
        println!("sample {name} {side} {} {} {sched} {} {}", self.w0, self.c0, self.c1, self.w1);
    }
}

fn trial(name: &str, body: impl FnOnce() -> u64) {
    let before = switches();
    let start = Sample::take();
    let batches = body();
    let end = Sample::take();
    let after = switches();
    start.emit(name, "start");
    end.emit(name, "end");
    println!("activity {name} {} {} {} {} {batches}", before[0], after[0], before[1], after[1]);
}

fn batch(mut value: u64) -> u64 {
    for i in 0..1024 {
        value = value.rotate_left(7).wrapping_add(i ^ value.wrapping_mul(17));
    }
    black_box(value)
}

fn use_thread_cpu() {
    let start = clock_ns(THREAD_CPU, false);
    let mut value = 1;
    while clock_ns(THREAD_CPU, false) - start < 40_000_000 {
        value = batch(value);
    }
}

enum Command { Delay, Contend }

fn main() {
    let cpu: u32 = std::env::args().nth(1).expect("CPU argument").parse().unwrap();
    identity("measured", cpu);
    println!("resolution {} {}", clock_ns(MONOTONIC, true), clock_ns(THREAD_CPU, true));
    let (send, receive) = mpsc::channel();
    let (ack, wait) = mpsc::channel();
    let batches = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let peer_batches = Arc::clone(&batches);
    let peer_stop = Arc::clone(&stop);
    let peer = thread::spawn(move || {
        identity("companion", cpu);
        ack.send(()).unwrap();
        for command in receive {
            match command {
                Command::Delay => {
                    thread::sleep(Duration::from_millis(80));
                    ack.send(()).unwrap();
                }
                Command::Contend => {
                    ack.send(()).unwrap();
                    let mut value = 2;
                    while !peer_stop.load(Ordering::Acquire) {
                        value = batch(value);
                        peer_batches.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    });
    wait.recv().unwrap();
    for i in 0..32 {
        trial(&format!("empty{i}"), || 0);
    }
    trial("cpu", || { use_thread_cpu(); 0 });
    trial("blocked", || {
        send.send(Command::Delay).unwrap();
        wait.recv().unwrap();
        0
    });
    send.send(Command::Contend).unwrap();
    wait.recv().unwrap();
    trial("contended", || {
        let before = batches.load(Ordering::Relaxed);
        use_thread_cpu();
        batches.load(Ordering::Relaxed) - before
    });
    stop.store(true, Ordering::Release);
    drop(send);
    peer.join().unwrap();
    println!("complete");
}
