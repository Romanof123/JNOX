# System time and timer service plan

[한국어](system-time-plan.md) · CHG-077 · [CAP-001 and CAP-012](platform-capabilities.en.md)

Existing AArch64 workloads read the physical counter and program its compare interrupt. EVD-014 observed PPI 30 preempt a running task and return to its saved PC on the Pi 4 model. EVD-017 exercised two virtual CPUs. These are starting points for a portable clock service. On single-core AArch64 QEMU virt the native SDK exposes a normal monotonic read to C and Rust applications at EL0. Realtime, RTC persistence, sleep and timer APIs remain planned. [Application clock evidence](evidence/evd-112.en.md) [Pi timer evidence](evidence/evd-014.en.md) · [SMP evidence](evidence/evd-017.en.md)

## Clock contract

| Clock | Origin and use | Update rules |
| --- | --- | --- |
| Monotonic | Boot-scoped, nondecreasing elapsed time for deadlines, relative waits and trace order within a qualified clock domain. Equal consecutive reads are valid. | Read-only. Wall-clock changes preserve its value and pending monotonic deadlines. First profile supports awake operation. |
| Realtime | Signed 64-bit POSIX seconds since the Epoch and normalized nanoseconds. Formatting and local time zones belong to applications. | Derive from a coherent monotonic reference and a versioned offset/rate. Track source, last synchronization, uncertainty and validity. An unknown date at boot remains explicitly unset. |
| Board RTC (optional) | A board driver can seed realtime at boot or persist an authorized date across power loss. | Validate source, range, oscillator/battery status when available and read consistency. Saving to RTC is a distinct privileged request. Record unavailable or rejected reads. |

The portable port contract supplies ordered counter reads, width, frequency, comparators, interrupt routing, suspend behavior and cross-core scope. On AArch64 use CNTPCT_EL0 and validated CNTFRQ_EL0, plus the nonsecure physical comparator where the BSP grants access. Keep raw ticks and boot/domain identity for evidence. Use checked wide conversion, round reads down and deadlines up to representable ticks, reject invalid nanoseconds and overflow including periodic rearm. Extend narrow counters only with a proven sampling/wrap horizon. Clock resolution, synchronization uncertainty and actual wake latency are separate measurements.

Publish one atomic snapshot of base ticks, offset, slew, validity and generation. A reader that races a writer retries against a consistent generation with a bounded kernel fallback. Verify common rate, epoch and ordered reads across migratable CPUs. If the port cannot establish global monotonicity, restrict the profile to a verified clock domain. On suspend, persist a coherent checkpoint and declare whether the counter stops. Enable resume only with verified continuity and an explicit elapsed-time convention. Reboot starts a new monotonic epoch. With a lost wall source, enter holdover, grow uncertainty by a characterized drift bound, and mark time unsynchronized when its limit expires or no bound exists.

## Access and expiry

Ordinary applications may read clocks and their quality. A configured owner grants separate rights to set realtime, request a bounded rate correction and write the RTC. Audit requester, source, previous/new generation, delta and outcome. No implicit synchronization from network or host. A step may move wall time backward. A slew converges under an explicit maximum rate with positive effective clock rate; serialize replacement or cancellation at the actual correction instant. Safety timeouts continue to use monotonic time. Unset realtime reads/absolute arms expose a documented unavailable status until a source is accepted.

Stage the selected POSIX surface: clock_gettime/getres, privileged clock_settime for realtime, clock_nanosleep with relative/absolute modes, then timer_create/settime/gettime/delete and overrun reporting. Publish clock IDs, error mappings, ABI range, interruption/restart behavior and finite object/notification capacity per profile. Absolute realtime waits re-evaluate after a step: forward crossings become due and backward steps postpone still-pending deadlines. Relative durations keep their original elapsed target. A periodic timer advances from its previous deadline, delivers bounded notification and reports missed periods. Serialize cancel/rearm/expiry by object generation; never deliver a stale generation to a reused handle. Record nominal deadline, actual IRQ observation, ready time and task return separately.

## First verification and decisions

1. With a deterministic counter backend, use a 3 Hz input and an independent integer oracle. Verify the first tick reads 333333333 ns and a 333333334 ns deadline arms tick 2. Exercise invalid frequency, wrap horizon, signed dates beyond 2038, overflow, denied setters and simultaneous cancel/expiry.
2. With monotonic at 10 s and realtime set to 100 s, arm a 5 s relative wait and an absolute realtime wait at 105 s. After 1 elapsed second step realtime to 110 s. Require the absolute wait due and the relative wait still due at monotonic 15 s. Repeat with a backward step, bounded slew, source loss, invalid RTC and reboot. Preserve each first result and raw inputs.
3. On AArch64 virt and the Pi model capture frequency/compare readback, actual IRQ/return and user timestamps from the exact image; check expiry ordering under load and per-core/migrated reads. Separately qualify physical oscillator drift, RTC power retention, suspend/resume, accuracy, overhead and independent clock-fault detection on the selected board.

**Product decisions** concern the trusted source/RTC, who may approve corrections, maximum step/slew and holdover uncertainty, suspend requirements and required synchronization accuracy. Engineering can implement monotonic and unset-wall operation, arithmetic and authorization tests before those selections. No product timing allocation follows from model ticks. [REQ-005/008/014/018](requirements.en.md) · [DES-003/004/005/006](architecture.en.md) · [VER-005/008/014/018](verification-plan.en.md)
